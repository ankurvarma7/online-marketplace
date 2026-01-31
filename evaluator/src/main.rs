use common::*;
use tokio::io::AsyncWriteExt;
use uuid::Uuid;
use std::time::{Instant, Duration};
use rand::{Rng, SeedableRng};
use rand::rngs::StdRng;
use tokio::task;

fn get_seller_server_addr() -> String {
    std::env::var("SELLER_SERVER_ADDR").unwrap_or_else(|_| "127.0.0.1:8082".to_string())
}

fn get_buyer_server_addr() -> String {
    std::env::var("BUYER_SERVER_ADDR").unwrap_or_else(|_| "127.0.0.1:8083".to_string())
}

#[derive(Clone)]
struct TestSession {
    seller_id: Uuid,
    seller_session: Uuid,
    buyer_id: Uuid,
    buyer_session: Uuid,
}

async fn send_seller_request(request: SellerRequest) -> Result<SellerResponse, Box<dyn std::error::Error + Send + Sync>> {
    use tokio::io::{AsyncBufReadExt, BufReader as TokioBufReader};
    let addr = get_seller_server_addr();
    let mut stream = tokio::net::TcpStream::connect(&addr).await?;
    
    let request_str = serde_json::to_string(&request)?;
    stream.write_all(request_str.as_bytes()).await?;
    stream.write_all(b"\n").await?;
    
    let mut response_str = String::new();
    let (reader, _) = stream.split();
    let mut buf_reader = TokioBufReader::new(reader);
    buf_reader.read_line(&mut response_str).await?;
    
    let response: SellerResponse = serde_json::from_str(response_str.trim())?;
    Ok(response)
}

async fn send_buyer_request(request: BuyerRequest) -> Result<BuyerResponse, Box<dyn std::error::Error + Send + Sync>> {
    use tokio::io::{AsyncBufReadExt, BufReader as TokioBufReader};
    let addr = get_buyer_server_addr();
    let mut stream = tokio::net::TcpStream::connect(&addr).await?;
    
    let request_str = serde_json::to_string(&request)?;
    stream.write_all(request_str.as_bytes()).await?;
    stream.write_all(b"\n").await?;
    
    let mut response_str = String::new();
    let (reader, _) = stream.split();
    let mut buf_reader = TokioBufReader::new(reader);
    buf_reader.read_line(&mut response_str).await?;
    
    let response: BuyerResponse = serde_json::from_str(response_str.trim())?;
    Ok(response)
}

async fn create_test_session(i: usize, run: usize) -> Result<TestSession, Box<dyn std::error::Error + Send + Sync>> {
    let seller_name = format!("seller_{}_{}", run, i);
    let buyer_name = format!("buyer_{}_{}", run, i);
    let password = "password".to_string();
    
    // Create seller account
    let seller_response = send_seller_request(SellerRequest::CreateAccount {
        seller_name: seller_name.clone(),
        password: password.clone(),
    }).await?;
    
    let seller_id = match seller_response {
        SellerResponse::CreateAccount(id) => id,
        _ => return Err("Failed to create seller account".into()),
    };
    
    // Login seller
    let seller_login_response = send_seller_request(SellerRequest::Login {
        seller_name,
        password: password.clone(),
    }).await?;
    
    let seller_session = match seller_login_response {
        SellerResponse::Login(session) => session,
        _ => return Err("Failed to login seller".into()),
    };
    
    // Create buyer account
    let buyer_response = send_buyer_request(BuyerRequest::CreateAccount {
        buyer_name: buyer_name.clone(),
        password: password.clone(),
    }).await?;
    
    let buyer_id = match buyer_response {
        BuyerResponse::CreateAccount(id) => id,
        _ => return Err("Failed to create buyer account".into()),
    };
    
    // Login buyer
    let buyer_login_response = send_buyer_request(BuyerRequest::Login {
        buyer_name,
        password,
    }).await?;
    
    let buyer_session = match buyer_login_response {
        BuyerResponse::Login(session) => session,
        _ => return Err("Failed to login buyer".into()),
    };
    
    Ok(TestSession {
        seller_id,
        seller_session,
        buyer_id,
        buyer_session,
    })
}

async fn run_single_test(run: usize) -> Result<(Duration, usize), Box<dyn std::error::Error + Send + Sync>> {
    let session = create_test_session(0, run).await?;
    let mut rng = StdRng::from_entropy();
    
    let start = Instant::now();
    let mut operations = 0;
    
    // Seller operations
    for i in 0..10 {
        let response = send_seller_request(SellerRequest::RegisterItemForSale {
            session_id: session.seller_session,
            item_name: format!("Item_{}", i),
            item_category: rng.gen_range(1..10),
            keywords: vec!["test".to_string(), "item".to_string()],
            condition: Condition::New,
            sale_price: rng.gen_range(10.0..100.0),
            quantity: rng.gen_range(1..100),
        }).await?;
        
        if let SellerResponse::RegisterItemForSale(_) = response {
            operations += 1;
        }
    }
    
    // Buyer operations
    for _ in 0..10 {
        let response = send_buyer_request(BuyerRequest::SearchItemsForSale {
            session_id: session.buyer_session,
            category: None,
            keywords: vec!["test".to_string()],
        }).await?;
        
        if let BuyerResponse::SearchItemsForSale(_) = response {
            operations += 1;
        }
    }
    
    let duration = start.elapsed();
    Ok((duration, operations))
}

async fn run_concurrent_test(num_users: usize, run: usize) -> Result<(Duration, usize), Box<dyn std::error::Error + Send + Sync>> {
    let mut handles = vec![];
    
    for i in 0..num_users {
        handles.push(task::spawn(async move {
            let mut operations = 0;
            let start = Instant::now();
            
            if let Ok(session) = create_test_session(i, run).await {
                let mut rng = StdRng::from_entropy();
                
                // Perform operations
                for _ in 0..10 {
                    // Seller operation
                    let _ = send_seller_request(SellerRequest::RegisterItemForSale {
                        session_id: session.seller_session,
                        item_name: format!("Item_{}_{}", i, rng.gen_range(0u32..=u32::MAX)),
                        item_category: rng.gen_range(1..10),
                        keywords: vec!["test".to_string()],
                        condition: Condition::New,
                        sale_price: rng.gen_range(10.0..100.0),
                        quantity: rng.gen_range(1..100),
                    }).await;
                    operations += 1;
                    
                    // Buyer operation
                    let _ = send_buyer_request(BuyerRequest::SearchItemsForSale {
                        session_id: session.buyer_session,
                        category: None,
                        keywords: vec!["test".to_string()],
                    }).await;
                    operations += 1;
                }
            }
            
            (start.elapsed(), operations)
        }));
    }
    
    let mut total_duration = Duration::new(0, 0);
    let mut total_operations = 0;
    
    for handle in handles {
        if let Ok((duration, ops)) = handle.await {
            total_duration = total_duration.max(duration); // Max duration for concurrent test
            total_operations += ops;
        }
    }
    
    Ok((total_duration, total_operations))
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    println!("Starting Performance Evaluation...");
    
    // Scenario 1: 1 seller, 1 buyer
    println!("\n=== Scenario 1: 1 seller, 1 buyer ===");
    let mut scenario1_times = Vec::new();
    let mut scenario1_throughputs = Vec::new();
    
    for run in 0..10 {
        println!("Run {}...", run + 1);
        let (duration, operations) = run_single_test(run).await?;
        let throughput = operations as f64 / duration.as_secs_f64();
        
        scenario1_times.push(duration);
        scenario1_throughputs.push(throughput);
        
        println!("  Duration: {:?}, Operations: {}, Throughput: {:.2} ops/sec", 
                duration, operations, throughput);
    }
    
    // Scenario 2: 10 concurrent sellers and buyers
    println!("\n=== Scenario 2: 10 sellers, 10 buyers ===");
    let mut scenario2_times = Vec::new();
    let mut scenario2_throughputs = Vec::new();
    
    for run in 0..10 {
        println!("Run {}...", run + 1);
        let (duration, operations) = run_concurrent_test(10, run).await?;
        let throughput = operations as f64 / duration.as_secs_f64();
        
        scenario2_times.push(duration);
        scenario2_throughputs.push(throughput);
        
        println!("  Duration: {:?}, Operations: {}, Throughput: {:.2} ops/sec", 
                duration, operations, throughput);
    }
    
    // Scenario 3: 100 concurrent sellers and buyers
    println!("\n=== Scenario 3: 100 sellers, 100 buyers ===");
    let mut scenario3_times = Vec::new();
    let mut scenario3_throughputs = Vec::new();
    
    for run in 0..10 {
        println!("Run {}...", run + 1);
        let (duration, operations) = run_concurrent_test(100, run).await?;
        let throughput = operations as f64 / duration.as_secs_f64();
        
        scenario3_times.push(duration);
        scenario3_throughputs.push(throughput);
        
        println!("  Duration: {:?}, Operations: {}, Throughput: {:.2} ops/sec", 
                duration, operations, throughput);
        
        // Add delay between runs to allow TIME_WAIT sockets to clear
        if run < 9 {
            println!("  Waiting 5 seconds for connections to clear...");
            tokio::time::sleep(tokio::time::Duration::from_secs(5)).await;
        }
    }
    
    // Calculate averages
    let avg_time1: Duration = scenario1_times.iter().sum::<Duration>() / scenario1_times.len() as u32;
    let avg_throughput1: f64 = scenario1_throughputs.iter().sum::<f64>() / scenario1_throughputs.len() as f64;
    
    let avg_time2: Duration = scenario2_times.iter().sum::<Duration>() / scenario2_times.len() as u32;
    let avg_throughput2: f64 = scenario2_throughputs.iter().sum::<f64>() / scenario2_throughputs.len() as f64;
    
    let avg_time3: Duration = scenario3_times.iter().sum::<Duration>() / scenario3_times.len() as u32;
    let avg_throughput3: f64 = scenario3_throughputs.iter().sum::<f64>() / scenario3_throughputs.len() as f64;
    
    // Print results
    println!("\n=== Results Summary ===");
    println!("Scenario 1 (1x1):");
    println!("  Average Response Time: {:?}", avg_time1);
    println!("  Average Throughput: {:.2} ops/sec", avg_throughput1);
    
    println!("\nScenario 2 (10x10):");
    println!("  Average Response Time: {:?}", avg_time2);
    println!("  Average Throughput: {:.2} ops/sec", avg_throughput2);
    
    println!("\nScenario 3 (100x100):");
    println!("  Average Response Time: {:?}", avg_time3);
    println!("  Average Throughput: {:.2} ops/sec", avg_throughput3);
    
    // Analysis
    println!("\n=== Performance Analysis ===");
    println!("1. Scenario 1 shows baseline performance with minimal contention.");
    println!("2. Scenario 2 demonstrates how the system handles moderate concurrency.");
    println!("3. Scenario 3 tests system limits with high concurrency.");
    println!("\nExpected observations:");
    println!("- Response time increases with more concurrent users");
    println!("- Throughput should increase from Scenario 1 to 2, but may plateau or decrease in Scenario 3");
    println!("- The system should remain stable under all scenarios");
    
    Ok(())
}