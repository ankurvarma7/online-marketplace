use common::*;
use tokio::net::{TcpListener, TcpStream};
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use uuid::Uuid;
use chrono::Utc;

fn get_customer_db_addr() -> String {
    std::env::var("CUSTOMER_DB_ADDR").unwrap_or_else(|_| "127.0.0.1:8080".to_string())
}

fn get_product_db_addr() -> String {
    std::env::var("PRODUCT_DB_ADDR").unwrap_or_else(|_| "127.0.0.1:8081".to_string())
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let bind_addr = std::env::var("SELLER_SERVER_BIND_ADDR").unwrap_or_else(|_| "127.0.0.1:8082".to_string());
    let listener = TcpListener::bind(&bind_addr).await?;
    println!("Seller Server listening on {}", bind_addr);
    
    loop {
        let (socket, _) = listener.accept().await?;
        tokio::spawn(async move {
            if let Err(e) = handle_connection(socket).await {
                eprintln!("Error handling connection: {}", e);
            }
        });
    }
}

async fn handle_connection(socket: TcpStream) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let (read_half, mut write_half) = socket.into_split();
    let reader = BufReader::new(read_half);
    let mut lines = reader.lines();
    
    while let Ok(Some(line)) = lines.next_line().await {
        let request: SellerRequest = match serde_json::from_str(&line) {
            Ok(req) => req,
            Err(e) => {
                let response = SellerResponse::Error(format!("Invalid request: {}", e));
                let _ = send_response(&mut write_half, response).await;
                continue;
            }
        };
        
        let response = handle_request(request).await;
        let _ = send_response(&mut write_half, response).await;
    }
    
    Ok(())
}

async fn handle_request(request: SellerRequest) -> SellerResponse {
    match request {
        SellerRequest::CreateAccount { seller_name, password } => {
            match send_to_customer_db(CustomerDbRequest::CreateSeller {
                seller_name: seller_name.clone(),
                password,
            }).await {
                Ok(CustomerDbResponse::SellerCreated(seller_id)) => {
                    SellerResponse::CreateAccount(seller_id)
                }
                Ok(CustomerDbResponse::Error(msg)) => SellerResponse::Error(msg),
                _ => SellerResponse::Error("Failed to create seller account".to_string()),
            }
        }
        
        SellerRequest::Login { seller_name, password } => {
            match send_to_customer_db(CustomerDbRequest::GetSellerByName {
                seller_name: seller_name.clone(),
            }).await {
                Ok(CustomerDbResponse::Seller(Some(seller))) => {
                    if seller.password == password {
                        match send_to_customer_db(CustomerDbRequest::CreateSession {
                            user_id: seller.seller_id,
                            user_type: UserType::Seller,
                        }).await {
                            Ok(CustomerDbResponse::SessionCreated(session_id, _)) => {
                                SellerResponse::Login(session_id)
                            }
                            Ok(CustomerDbResponse::Error(msg)) => SellerResponse::Error(msg),
                            _ => SellerResponse::Error("Failed to create session".to_string()),
                        }
                    } else {
                        SellerResponse::Error("Invalid password".to_string())
                    }
                }
                Ok(CustomerDbResponse::Seller(None)) => {
                    SellerResponse::Error("Seller not found".to_string())
                }
                Ok(CustomerDbResponse::Error(msg)) => SellerResponse::Error(msg),
                _ => SellerResponse::Error("Login failed".to_string()),
            }
        }
        
        SellerRequest::Logout { session_id } => {
            match send_to_customer_db(CustomerDbRequest::DeleteSession { session_id }).await {
                Ok(CustomerDbResponse::SessionDeleted) => SellerResponse::Logout,
                Ok(CustomerDbResponse::Error(msg)) => SellerResponse::Error(msg),
                _ => SellerResponse::Error("Logout failed".to_string()),
            }
        }
        
        SellerRequest::GetSellerRating { session_id } => {
            match validate_session(session_id, UserType::Seller).await {
                Ok(session) => {
                    match send_to_customer_db(CustomerDbRequest::GetSeller {
                        seller_id: session.user_id,
                    }).await {
                        Ok(CustomerDbResponse::Seller(Some(seller))) => {
                            SellerResponse::GetSellerRating(seller.feedback)
                        }
                        Ok(CustomerDbResponse::Seller(None)) => {
                            SellerResponse::Error("Seller not found".to_string())
                        }
                        Ok(CustomerDbResponse::Error(msg)) => SellerResponse::Error(msg),
                        _ => SellerResponse::Error("Failed to get seller rating".to_string()),
                    }
                }
                Err(e) => SellerResponse::Error(e),
            }
        }
        
        SellerRequest::RegisterItemForSale {
            session_id,
            item_name,
            item_category,
            keywords,
            condition,
            sale_price,
            quantity,
        } => {
            match validate_session(session_id, UserType::Seller).await {
                Ok(session) => {
                    let item = Item {
                        item_id: Uuid::nil(), // Will be assigned by product DB
                        item_name,
                        item_category,
                        keywords,
                        condition,
                        sale_price,
                        quantity,
                        feedback: Feedback { thumbs_up: 0, thumbs_down: 0 },
                        seller_id: session.user_id,
                    };
                    
                    match send_to_product_db(ProductDbRequest::CreateItem { item }).await {
                        Ok(ProductDbResponse::ItemCreated(item_id)) => {
                            SellerResponse::RegisterItemForSale(item_id)
                        }
                        Ok(ProductDbResponse::Error(msg)) => SellerResponse::Error(msg),
                        _ => SellerResponse::Error("Failed to register item".to_string()),
                    }
                }
                Err(e) => SellerResponse::Error(e),
            }
        }
        
        SellerRequest::ChangeItemPrice { session_id, item_id, new_price } => {
            match validate_session(session_id, UserType::Seller).await {
                Ok(session) => {
                    match send_to_product_db(ProductDbRequest::GetItem { item_id }).await {
                        Ok(ProductDbResponse::Item(Some(mut item))) => {
                            if item.seller_id != session.user_id {
                                return SellerResponse::Error("Not your item".to_string());
                            }
                            
                            item.sale_price = new_price;
                            
                            match send_to_product_db(ProductDbRequest::UpdateItem { item }).await {
                                Ok(ProductDbResponse::ItemUpdated) => SellerResponse::ChangeItemPrice,
                                Ok(ProductDbResponse::Error(msg)) => SellerResponse::Error(msg),
                                _ => SellerResponse::Error("Failed to update price".to_string()),
                            }
                        }
                        Ok(ProductDbResponse::Item(None)) => {
                            SellerResponse::Error("Item not found".to_string())
                        }
                        Ok(ProductDbResponse::Error(msg)) => SellerResponse::Error(msg),
                        _ => SellerResponse::Error("Failed to get item".to_string()),
                    }
                }
                Err(e) => SellerResponse::Error(e),
            }
        }
        
        SellerRequest::UpdateUnitsForSale { session_id, item_id, quantity } => {
            match validate_session(session_id, UserType::Seller).await {
                Ok(session) => {
                    match send_to_product_db(ProductDbRequest::GetItem { item_id }).await {
                        Ok(ProductDbResponse::Item(Some(mut item))) => {
                            if item.seller_id != session.user_id {
                                return SellerResponse::Error("Not your item".to_string());
                            }
                            
                            item.quantity = quantity;
                            
                            match send_to_product_db(ProductDbRequest::UpdateItem { item }).await {
                                Ok(ProductDbResponse::ItemUpdated) => SellerResponse::UpdateUnitsForSale,
                                Ok(ProductDbResponse::Error(msg)) => SellerResponse::Error(msg),
                                _ => SellerResponse::Error("Failed to update quantity".to_string()),
                            }
                        }
                        Ok(ProductDbResponse::Item(None)) => {
                            SellerResponse::Error("Item not found".to_string())
                        }
                        Ok(ProductDbResponse::Error(msg)) => SellerResponse::Error(msg),
                        _ => SellerResponse::Error("Failed to get item".to_string()),
                    }
                }
                Err(e) => SellerResponse::Error(e),
            }
        }
        
        SellerRequest::DisplayItemsForSale { session_id } => {
            match validate_session(session_id, UserType::Seller).await {
                Ok(session) => {
                    match send_to_product_db(ProductDbRequest::GetItemsBySeller {
                        seller_id: session.user_id,
                    }).await {
                        Ok(ProductDbResponse::Items(items)) => {
                            SellerResponse::DisplayItemsForSale(items)
                        }
                        Ok(ProductDbResponse::Error(msg)) => SellerResponse::Error(msg),
                        _ => SellerResponse::Error("Failed to get items".to_string()),
                    }
                }
                Err(e) => SellerResponse::Error(e),
            }
        }
    }
}

async fn validate_session(session_id: Uuid, expected_type: UserType) -> Result<Session, String> {
    match send_to_customer_db(CustomerDbRequest::GetSession { session_id }).await {
        Ok(CustomerDbResponse::Session(Some(session))) => {
            let now = Utc::now().timestamp();
            
            if session.expiration < now {
                let _ = send_to_customer_db(CustomerDbRequest::DeleteSession { session_id }).await;
                return Err("Session expired".to_string());
            }
            
            if session.user_type != expected_type {
                return Err("Invalid session type".to_string());
            }
            
            Ok(session)
        }
        Ok(CustomerDbResponse::Session(None)) => Err("Session not found".to_string()),
        Ok(CustomerDbResponse::Error(msg)) => Err(msg),
        _ => Err("Failed to validate session".to_string()),
    }
}

async fn send_to_customer_db(request: CustomerDbRequest) -> Result<CustomerDbResponse, Box<dyn std::error::Error + Send + Sync>> {
    let addr = get_customer_db_addr();
    let mut stream = tokio::net::TcpStream::connect(&addr).await?;
    send_and_receive(&mut stream, request).await
}

async fn send_to_product_db(request: ProductDbRequest) -> Result<ProductDbResponse, Box<dyn std::error::Error + Send + Sync>> {
    let addr = get_product_db_addr();
    let mut stream = tokio::net::TcpStream::connect(&addr).await?;
    send_and_receive(&mut stream, request).await
}

async fn send_and_receive<T, U>(
    stream: &mut tokio::net::TcpStream,
    request: T,
) -> Result<U, Box<dyn std::error::Error + Send + Sync>>
where
    T: serde::Serialize,
    U: for<'de> serde::Deserialize<'de>,
{
    let request_str = serde_json::to_string(&request)?;
    stream.write_all(request_str.as_bytes()).await?;
    stream.write_all(b"\n").await?;

    let mut response_str = String::new();
    let (reader, _) = stream.split();
    let mut buf_reader = BufReader::new(reader);
    buf_reader.read_line(&mut response_str).await?;

    let response: U = serde_json::from_str(response_str.trim())?;
    Ok(response)
}

async fn send_response(
    writer: &mut tokio::net::tcp::OwnedWriteHalf,
    response: SellerResponse,
) -> Result<(), Box<dyn std::error::Error>> {
    let response_str = serde_json::to_string(&response)?;
    writer.write_all(response_str.as_bytes()).await?;
    writer.write_all(b"\n").await?;
    Ok(())
}