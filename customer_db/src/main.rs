use common::*;
use dashmap::DashMap;
use std::sync::Arc;
use tokio::net::{TcpListener, TcpStream};
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use uuid::Uuid;
use chrono::Utc;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let bind_addr = std::env::var("CUSTOMER_DB_BIND_ADDR").unwrap_or_else(|_| "127.0.0.1:8080".to_string());
    let listener = TcpListener::bind(&bind_addr).await?;
    println!("Customer Database listening on {}", bind_addr);
    
    // In-memory storage
    let sellers: Arc<DashMap<Uuid, Seller>> = Arc::new(DashMap::new());
    let buyers: Arc<DashMap<Uuid, Buyer>> = Arc::new(DashMap::new());
    let sessions: Arc<DashMap<Uuid, Session>> = Arc::new(DashMap::new());
    
    // Background session cleaner
    let sessions_clone = sessions.clone();
    tokio::spawn(async move {
        cleanup_sessions(sessions_clone).await;
    });
    
    loop {
        let (socket, _) = listener.accept().await?;
        let sellers_clone = sellers.clone();
        let buyers_clone = buyers.clone();
        let sessions_clone = sessions.clone();
        
        tokio::spawn(async move {
            handle_connection(socket, sellers_clone, buyers_clone, sessions_clone).await;
        });
    }
}

async fn handle_connection(
    socket: TcpStream,
    sellers: Arc<DashMap<Uuid, Seller>>,
    buyers: Arc<DashMap<Uuid, Buyer>>,
    sessions: Arc<DashMap<Uuid, Session>>,
) {
    let (read_half, mut write_half) = socket.into_split();
    let reader = BufReader::new(read_half);
    let mut lines = reader.lines();
    
    while let Ok(Some(line)) = lines.next_line().await {
        let request: CustomerDbRequest = match serde_json::from_str(&line) {
            Ok(req) => req,
            Err(e) => {
                let response = CustomerDbResponse::Error(format!("Invalid request: {}", e));
                let _ = send_response(&mut write_half, response).await;
                continue;
            }
        };
        
        let response = handle_request(request, &sellers, &buyers, &sessions).await;
        let _ = send_response(&mut write_half, response).await;
    }
}

async fn handle_request(
    request: CustomerDbRequest,
    sellers: &DashMap<Uuid, Seller>,
    buyers: &DashMap<Uuid, Buyer>,
    sessions: &DashMap<Uuid, Session>,
) -> CustomerDbResponse {
    match request {
        CustomerDbRequest::CreateSeller { seller_name, password } => {
            let seller_id = Uuid::new_v4();
            let seller = Seller {
                seller_id,
                seller_name,
                feedback: Feedback { thumbs_up: 0, thumbs_down: 0 },
                items_sold: 0,
                password,
            };
            sellers.insert(seller_id, seller);
            CustomerDbResponse::SellerCreated(seller_id)
        }
        
        CustomerDbRequest::CreateBuyer { buyer_name, password } => {
            let buyer_id = Uuid::new_v4();
            let buyer = Buyer {
                buyer_id,
                buyer_name,
                items_purchased: 0,
                password,
            };
            buyers.insert(buyer_id, buyer);
            CustomerDbResponse::BuyerCreated(buyer_id)
        }
        
        CustomerDbRequest::GetSellerByName { seller_name } => {
            let seller = sellers.iter()
                .find(|s| s.seller_name == seller_name)
                .map(|s| s.value().clone());
            CustomerDbResponse::Seller(seller)
        }
        
        CustomerDbRequest::GetBuyerByName { buyer_name } => {
            let buyer = buyers.iter()
                .find(|b| b.buyer_name == buyer_name)
                .map(|b| b.value().clone());
            CustomerDbResponse::Buyer(buyer)
        }
        
        CustomerDbRequest::GetSeller { seller_id } => {
            let seller = sellers.get(&seller_id).map(|s| s.clone());
            CustomerDbResponse::Seller(seller)
        }
        
        CustomerDbRequest::UpdateSeller { seller } => {
            sellers.insert(seller.seller_id, seller);
            CustomerDbResponse::SellerUpdated
        }
        
        CustomerDbRequest::GetBuyer { buyer_id } => {
            let buyer = buyers.get(&buyer_id).map(|b| b.clone());
            CustomerDbResponse::Buyer(buyer)
        }
        
        CustomerDbRequest::UpdateBuyer { buyer } => {
            buyers.insert(buyer.buyer_id, buyer);
            CustomerDbResponse::BuyerUpdated
        }
        
        CustomerDbRequest::CreateSession { user_id, user_type } => {
            let session_id = Uuid::new_v4();
            let expiration = Utc::now().timestamp() + 300; // 5 minutes
            let session = Session {
                session_id,
                user_id,
                user_type,
                expiration,
            };
            sessions.insert(session_id, session);
            CustomerDbResponse::SessionCreated(session_id, expiration)
        }
        
        CustomerDbRequest::GetSession { session_id } => {
            let session = sessions.get(&session_id).map(|s| s.clone());
            CustomerDbResponse::Session(session)
        }
        
        CustomerDbRequest::DeleteSession { session_id } => {
            sessions.remove(&session_id);
            CustomerDbResponse::SessionDeleted
        }
        
        CustomerDbRequest::CleanupSessions => {
            let now = Utc::now().timestamp();
            let expired: Vec<Uuid> = sessions.iter()
                .filter(|s| s.expiration < now)
                .map(|s| s.session_id)
                .collect();
            
            for session_id in &expired {
                sessions.remove(session_id);
            }
            
            CustomerDbResponse::SessionsCleaned(expired.len())
        }
    }
}

async fn send_response(
    writer: &mut tokio::net::tcp::OwnedWriteHalf,
    response: CustomerDbResponse,
) -> Result<(), Box<dyn std::error::Error>> {
    let response_str = serde_json::to_string(&response)?;
    writer.write_all(response_str.as_bytes()).await?;
    writer.write_all(b"\n").await?;
    Ok(())
}

async fn cleanup_sessions(sessions: Arc<DashMap<Uuid, Session>>) {
    loop {
        tokio::time::sleep(tokio::time::Duration::from_secs(60)).await;
        
        let now = Utc::now().timestamp();
        let expired: Vec<Uuid> = sessions.iter()
            .filter(|s| s.expiration < now)
            .map(|s| s.session_id)
            .collect();
        
        for session_id in &expired {
            sessions.remove(session_id);
        }
        
        if !expired.is_empty() {
            println!("Cleaned up {} expired sessions", expired.len());
        }
    }
}