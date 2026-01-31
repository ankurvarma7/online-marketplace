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
    let bind_addr = std::env::var("BUYER_SERVER_BIND_ADDR").unwrap_or_else(|_| "127.0.0.1:8083".to_string());
    let listener = TcpListener::bind(&bind_addr).await?;
    println!("Buyer Server listening on {}", bind_addr);
    
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
        let request: BuyerRequest = match serde_json::from_str(&line) {
            Ok(req) => req,
            Err(e) => {
                let response = BuyerResponse::Error(format!("Invalid request: {}", e));
                let _ = send_response(&mut write_half, response).await;
                continue;
            }
        };
        
        let response = handle_request(request).await;
        let _ = send_response(&mut write_half, response).await;
    }
    
    Ok(())
}

async fn handle_request(request: BuyerRequest) -> BuyerResponse {
    match request {
        BuyerRequest::CreateAccount { buyer_name, password } => {
            match send_to_customer_db(CustomerDbRequest::CreateBuyer {
                buyer_name: buyer_name.clone(),
                password,
            }).await {
                Ok(CustomerDbResponse::BuyerCreated(buyer_id)) => {
                    BuyerResponse::CreateAccount(buyer_id)
                }
                Ok(CustomerDbResponse::Error(msg)) => BuyerResponse::Error(msg),
                _ => BuyerResponse::Error("Failed to create buyer account".to_string()),
            }
        }
        
        BuyerRequest::Login { buyer_name, password } => {
            match send_to_customer_db(CustomerDbRequest::GetBuyerByName {
                buyer_name: buyer_name.clone(),
            }).await {
                Ok(CustomerDbResponse::Buyer(Some(buyer))) => {
                    if buyer.password == password {
                        match send_to_customer_db(CustomerDbRequest::CreateSession {
                            user_id: buyer.buyer_id,
                            user_type: UserType::Buyer,
                        }).await {
                            Ok(CustomerDbResponse::SessionCreated(session_id, _)) => {
                                BuyerResponse::Login(session_id)
                            }
                            Ok(CustomerDbResponse::Error(msg)) => BuyerResponse::Error(msg),
                            _ => BuyerResponse::Error("Failed to create session".to_string()),
                        }
                    } else {
                        BuyerResponse::Error("Invalid password".to_string())
                    }
                }
                Ok(CustomerDbResponse::Buyer(None)) => {
                    BuyerResponse::Error("Buyer not found".to_string())
                }
                Ok(CustomerDbResponse::Error(msg)) => BuyerResponse::Error(msg),
                _ => BuyerResponse::Error("Login failed".to_string()),
            }
        }
        
        BuyerRequest::Logout { session_id } => {
            match send_to_customer_db(CustomerDbRequest::DeleteSession { session_id }).await {
                Ok(CustomerDbResponse::SessionDeleted) => BuyerResponse::Logout,
                Ok(CustomerDbResponse::Error(msg)) => BuyerResponse::Error(msg),
                _ => BuyerResponse::Error("Logout failed".to_string()),
            }
        }
        
        BuyerRequest::SearchItemsForSale { session_id, category, keywords } => {
            match validate_session(session_id, UserType::Buyer).await {
                Ok(_) => {
                    match send_to_product_db(ProductDbRequest::SearchItems {
                        category,
                        keywords,
                    }).await {
                        Ok(ProductDbResponse::Items(items)) => {
                            BuyerResponse::SearchItemsForSale(items)
                        }
                        Ok(ProductDbResponse::Error(msg)) => BuyerResponse::Error(msg),
                        _ => BuyerResponse::Error("Search failed".to_string()),
                    }
                }
                Err(e) => BuyerResponse::Error(e),
            }
        }
        
        BuyerRequest::GetItem { session_id, item_id } => {
            match validate_session(session_id, UserType::Buyer).await {
                Ok(_) => {
                    match send_to_product_db(ProductDbRequest::GetItem { item_id }).await {
                        Ok(ProductDbResponse::Item(item)) => BuyerResponse::GetItem(item),
                        Ok(ProductDbResponse::Error(msg)) => BuyerResponse::Error(msg),
                        _ => BuyerResponse::Error("Failed to get item".to_string()),
                    }
                }
                Err(e) => BuyerResponse::Error(e),
            }
        }
        
        BuyerRequest::AddItemToCart { session_id, item_id, quantity } => {
            match validate_session(session_id, UserType::Buyer).await {
                Ok(session) => {
                    match send_to_product_db(ProductDbRequest::AddToCart {
                        buyer_id: session.user_id,
                        item_id,
                        quantity,
                    }).await {
                        Ok(ProductDbResponse::CartSaved) => BuyerResponse::AddItemToCart,
                        Ok(ProductDbResponse::Error(msg)) => BuyerResponse::Error(msg),
                        _ => BuyerResponse::Error("Failed to add to cart".to_string()),
                    }
                }
                Err(e) => BuyerResponse::Error(e),
            }
        }
        
        BuyerRequest::RemoveItemFromCart { session_id, item_id, quantity } => {
            match validate_session(session_id, UserType::Buyer).await {
                Ok(session) => {
                    match send_to_product_db(ProductDbRequest::RemoveFromCart {
                        buyer_id: session.user_id,
                        item_id,
                        quantity,
                    }).await {
                        Ok(ProductDbResponse::CartSaved) => BuyerResponse::RemoveItemFromCart,
                        Ok(ProductDbResponse::Error(msg)) => BuyerResponse::Error(msg),
                        _ => BuyerResponse::Error("Failed to remove from cart".to_string()),
                    }
                }
                Err(e) => BuyerResponse::Error(e),
            }
        }
        
        BuyerRequest::SaveCart { session_id } => {
            match validate_session(session_id, UserType::Buyer).await {
                Ok(session) => {
                    match send_to_product_db(ProductDbRequest::GetCart {
                        buyer_id: session.user_id,
                    }).await {
                        Ok(ProductDbResponse::Cart(cart)) => {
                            match send_to_product_db(ProductDbRequest::SaveCart {
                                buyer_id: session.user_id,
                                cart,
                            }).await {
                                Ok(ProductDbResponse::CartSaved) => BuyerResponse::SaveCart,
                                Ok(ProductDbResponse::Error(msg)) => BuyerResponse::Error(msg),
                                _ => BuyerResponse::Error("Failed to save cart".to_string()),
                            }
                        }
                        Ok(ProductDbResponse::Error(msg)) => BuyerResponse::Error(msg),
                        _ => BuyerResponse::Error("Failed to get cart".to_string()),
                    }
                }
                Err(e) => BuyerResponse::Error(e),
            }
        }
        
        BuyerRequest::ClearCart { session_id } => {
            match validate_session(session_id, UserType::Buyer).await {
                Ok(session) => {
                    match send_to_product_db(ProductDbRequest::ClearCart {
                        buyer_id: session.user_id,
                    }).await {
                        Ok(ProductDbResponse::CartCleared) => BuyerResponse::ClearCart,
                        Ok(ProductDbResponse::Error(msg)) => BuyerResponse::Error(msg),
                        _ => BuyerResponse::Error("Failed to clear cart".to_string()),
                    }
                }
                Err(e) => BuyerResponse::Error(e),
            }
        }
        
        BuyerRequest::DisplayCart { session_id } => {
            match validate_session(session_id, UserType::Buyer).await {
                Ok(session) => {
                    match send_to_product_db(ProductDbRequest::GetCart {
                        buyer_id: session.user_id,
                    }).await {
                        Ok(ProductDbResponse::Cart(cart)) => BuyerResponse::DisplayCart(cart),
                        Ok(ProductDbResponse::Error(msg)) => BuyerResponse::Error(msg),
                        _ => BuyerResponse::Error("Failed to get cart".to_string()),
                    }
                }
                Err(e) => BuyerResponse::Error(e),
            }
        }
        
        BuyerRequest::ProvideFeedback { session_id, item_id, thumbs_up } => {
            match validate_session(session_id, UserType::Buyer).await {
                Ok(_) => {
                    match send_to_product_db(ProductDbRequest::GetItem { item_id }).await {
                        Ok(ProductDbResponse::Item(Some(mut item))) => {
                            if thumbs_up {
                                item.feedback.thumbs_up += 1;
                            } else {
                                item.feedback.thumbs_down += 1;
                            }
                            
                            match send_to_product_db(ProductDbRequest::UpdateItem { item }).await {
                                Ok(ProductDbResponse::ItemUpdated) => BuyerResponse::ProvideFeedback,
                                Ok(ProductDbResponse::Error(msg)) => BuyerResponse::Error(msg),
                                _ => BuyerResponse::Error("Failed to update feedback".to_string()),
                            }
                        }
                        Ok(ProductDbResponse::Item(None)) => {
                            BuyerResponse::Error("Item not found".to_string())
                        }
                        Ok(ProductDbResponse::Error(msg)) => BuyerResponse::Error(msg),
                        _ => BuyerResponse::Error("Failed to get item".to_string()),
                    }
                }
                Err(e) => BuyerResponse::Error(e),
            }
        }
        
        BuyerRequest::GetSellerRating { session_id, seller_id } => {
            match validate_session(session_id, UserType::Buyer).await {
                Ok(_) => {
                    match send_to_customer_db(CustomerDbRequest::GetSeller { seller_id }).await {
                        Ok(CustomerDbResponse::Seller(Some(seller))) => {
                            BuyerResponse::GetSellerRating(seller.feedback)
                        }
                        Ok(CustomerDbResponse::Seller(None)) => {
                            BuyerResponse::Error("Seller not found".to_string())
                        }
                        Ok(CustomerDbResponse::Error(msg)) => BuyerResponse::Error(msg),
                        _ => BuyerResponse::Error("Failed to get seller rating".to_string()),
                    }
                }
                Err(e) => BuyerResponse::Error(e),
            }
        }
        
        BuyerRequest::GetBuyerPurchases { session_id } => {
            match validate_session(session_id, UserType::Buyer).await {
                Ok(session) => {
                    match send_to_product_db(ProductDbRequest::GetPurchaseHistory {
                        buyer_id: session.user_id,
                    }).await {
                        Ok(ProductDbResponse::PurchaseHistory(history)) => {
                            BuyerResponse::GetBuyerPurchases(history)
                        }
                        Ok(ProductDbResponse::Error(msg)) => BuyerResponse::Error(msg),
                        _ => BuyerResponse::Error("Failed to get purchase history".to_string()),
                    }
                }
                Err(e) => BuyerResponse::Error(e),
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
    response: BuyerResponse,
) -> Result<(), Box<dyn std::error::Error>> {
    let response_str = serde_json::to_string(&response)?;
    writer.write_all(response_str.as_bytes()).await?;
    writer.write_all(b"\n").await?;
    Ok(())
}