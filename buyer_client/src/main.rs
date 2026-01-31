use clap::{Parser, Subcommand};
use common::*;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use uuid::Uuid;

fn get_buyer_server_addr() -> String {
    std::env::var("BUYER_SERVER_ADDR").unwrap_or_else(|_| "127.0.0.1:8083".to_string())
}

#[derive(Parser)]
#[command(name = "buyer_client")]
#[command(about = "Online Marketplace Buyer Client")]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Create a new buyer account
    CreateAccount {
        #[arg(short, long)]
        name: String,
        #[arg(short, long)]
        password: String,
    },
    /// Login to buyer account
    Login {
        #[arg(short, long)]
        name: String,
        #[arg(short, long)]
        password: String,
    },
    /// Logout from current session
    Logout {
        #[arg(short, long)]
        session_id: String,
    },
    /// Search items for sale
    Search {
        #[arg(short, long)]
        session_id: String,
        #[arg(short, long)]
        category: Option<i32>,
        #[arg(short, long, num_args = 0..=5, value_delimiter = ',')]
        keywords: Vec<String>,
    },
    /// Get item details
    GetItem {
        #[arg(short, long)]
        session_id: String,
        #[arg(short, long)]
        item_id: String,
    },
    /// Add item to cart
    AddToCart {
        #[arg(short, long)]
        session_id: String,
        #[arg(short, long)]
        item_id: String,
        #[arg(short, long)]
        quantity: i32,
    },
    /// Remove item from cart
    RemoveFromCart {
        #[arg(short, long)]
        session_id: String,
        #[arg(short, long)]
        item_id: String,
        #[arg(short, long)]
        quantity: i32,
    },
    /// Save cart
    SaveCart {
        #[arg(short, long)]
        session_id: String,
    },
    /// Clear cart
    ClearCart {
        #[arg(short, long)]
        session_id: String,
    },
    /// Display cart
    DisplayCart {
        #[arg(short, long)]
        session_id: String,
    },
    /// Provide feedback for item
    Feedback {
        #[arg(short, long)]
        session_id: String,
        #[arg(short, long)]
        item_id: String,
        #[arg(short, long)]
        thumbs_up: bool,
    },
    /// Get seller rating
    GetSellerRating {
        #[arg(short, long)]
        session_id: String,
        #[arg(short, long)]
        seller_id: String,
    },
    /// Get purchase history
    GetPurchases {
        #[arg(short, long)]
        session_id: String,
    },
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let cli = Cli::parse();
    
    match cli.command {
        Commands::CreateAccount { name, password } => {
            create_account(name, password).await?;
        }
        Commands::Login { name, password } => {
            login(name, password).await?;
        }
        Commands::Logout { session_id } => {
            logout(session_id).await?;
        }
        Commands::Search {
            session_id,
            category,
            keywords,
        } => {
            search(session_id, category, keywords).await?;
        }
        Commands::GetItem { session_id, item_id } => {
            get_item(session_id, item_id).await?;
        }
        Commands::AddToCart {
            session_id,
            item_id,
            quantity,
        } => {
            add_to_cart(session_id, item_id, quantity).await?;
        }
        Commands::RemoveFromCart {
            session_id,
            item_id,
            quantity,
        } => {
            remove_from_cart(session_id, item_id, quantity).await?;
        }
        Commands::SaveCart { session_id } => {
            save_cart(session_id).await?;
        }
        Commands::ClearCart { session_id } => {
            clear_cart(session_id).await?;
        }
        Commands::DisplayCart { session_id } => {
            display_cart(session_id).await?;
        }
        Commands::Feedback {
            session_id,
            item_id,
            thumbs_up,
        } => {
            feedback(session_id, item_id, thumbs_up).await?;
        }
        Commands::GetSellerRating {
            session_id,
            seller_id,
        } => {
            get_seller_rating(session_id, seller_id).await?;
        }
        Commands::GetPurchases { session_id } => {
            get_purchases(session_id).await?;
        }
    }
    
    Ok(())
}

async fn send_request(request: BuyerRequest) -> Result<BuyerResponse, Box<dyn std::error::Error>> {
    let addr = get_buyer_server_addr();
    let mut stream = tokio::net::TcpStream::connect(&addr).await?;
    
    let request_str = serde_json::to_string(&request)?;
    stream.write_all(request_str.as_bytes()).await?;
    stream.write_all(b"\n").await?;
    
    let mut response_str = String::new();
    let mut reader = BufReader::new(stream);
    reader.read_line(&mut response_str).await?;
    
    let response: BuyerResponse = serde_json::from_str(&response_str)?;
    Ok(response)
}

async fn create_account(name: String, password: String) -> Result<(), Box<dyn std::error::Error>> {
    let request = BuyerRequest::CreateAccount {
        buyer_name: name,
        password,
    };
    
    match send_request(request).await? {
        BuyerResponse::CreateAccount(buyer_id) => {
            println!("Account created successfully!");
            println!("Buyer ID: {}", buyer_id);
            Ok(())
        }
        BuyerResponse::Error(msg) => {
            eprintln!("Error: {}", msg);
            Ok(())
        }
        _ => {
            eprintln!("Unexpected response");
            Ok(())
        }
    }
}

async fn login(name: String, password: String) -> Result<(), Box<dyn std::error::Error>> {
    let request = BuyerRequest::Login {
        buyer_name: name,
        password,
    };
    
    match send_request(request).await? {
        BuyerResponse::Login(session_id) => {
            println!("Login successful!");
            println!("Session ID: {}", session_id);
            println!("Session expires in 5 minutes");
            Ok(())
        }
        BuyerResponse::Error(msg) => {
            eprintln!("Error: {}", msg);
            Ok(())
        }
        _ => {
            eprintln!("Unexpected response");
            Ok(())
        }
    }
}

async fn logout(session_id_str: String) -> Result<(), Box<dyn std::error::Error>> {
    let session_id = Uuid::parse_str(&session_id_str)?;
    
    let request = BuyerRequest::Logout { session_id };
    
    match send_request(request).await? {
        BuyerResponse::Logout => {
            println!("Logout successful!");
            Ok(())
        }
        BuyerResponse::Error(msg) => {
            eprintln!("Error: {}", msg);
            Ok(())
        }
        _ => {
            eprintln!("Unexpected response");
            Ok(())
        }
    }
}

async fn search(
    session_id_str: String,
    category: Option<i32>,
    keywords: Vec<String>,
) -> Result<(), Box<dyn std::error::Error>> {
    let session_id = Uuid::parse_str(&session_id_str)?;
    
    // Trim keywords to match seller_client behavior
    let keywords: Vec<String> = keywords.into_iter()
        .map(|k| {
            let k = k.trim().to_string();
            if k.len() > 8 {
                k[..8].to_string()
            } else {
                k
            }
        })
        .take(5)
        .collect();
    
    let request = BuyerRequest::SearchItemsForSale {
        session_id,
        category,
        keywords,
    };
    
    match send_request(request).await? {
        BuyerResponse::SearchItemsForSale(items) => {
            if items.is_empty() {
                println!("No items found.");
                return Ok(());
            }
            
            println!("Search Results ({} items):", items.len());
            println!("{:-<80}", "");
            for item in items {
                println!("Item ID: {}", item.item_id);
                println!("  Name: {}", item.item_name);
                println!("  Category: {}", item.item_category);
                println!("  Keywords: {}", item.keywords.join(", "));
                println!("  Condition: {:?}", item.condition);
                println!("  Price: ${:.2}", item.sale_price);
                println!("  Quantity: {}", item.quantity);
                println!("  Feedback: ↑{} ↓{}", item.feedback.thumbs_up, item.feedback.thumbs_down);
                println!("{:-<80}", "");
            }
            Ok(())
        }
        BuyerResponse::Error(msg) => {
            eprintln!("Error: {}", msg);
            Ok(())
        }
        _ => {
            eprintln!("Unexpected response");
            Ok(())
        }
    }
}

async fn get_item(session_id_str: String, item_id_str: String) -> Result<(), Box<dyn std::error::Error>> {
    let session_id = Uuid::parse_str(&session_id_str)?;
    let item_id = Uuid::parse_str(&item_id_str)?;
    
    let request = BuyerRequest::GetItem {
        session_id,
        item_id,
    };
    
    match send_request(request).await? {
        BuyerResponse::GetItem(Some(item)) => {
            println!("Item Details:");
            println!("  ID: {}", item.item_id);
            println!("  Name: {}", item.item_name);
            println!("  Category: {}", item.item_category);
            println!("  Keywords: {}", item.keywords.join(", "));
            println!("  Condition: {:?}", item.condition);
            println!("  Price: ${:.2}", item.sale_price);
            println!("  Quantity: {}", item.quantity);
            println!("  Feedback: ↑{} ↓{}", item.feedback.thumbs_up, item.feedback.thumbs_down);
            Ok(())
        }
        BuyerResponse::GetItem(None) => {
            println!("Item not found.");
            Ok(())
        }
        BuyerResponse::Error(msg) => {
            eprintln!("Error: {}", msg);
            Ok(())
        }
        _ => {
            eprintln!("Unexpected response");
            Ok(())
        }
    }
}

async fn add_to_cart(
    session_id_str: String,
    item_id_str: String,
    quantity: i32,
) -> Result<(), Box<dyn std::error::Error>> {
    let session_id = Uuid::parse_str(&session_id_str)?;
    let item_id = Uuid::parse_str(&item_id_str)?;
    
    let request = BuyerRequest::AddItemToCart {
        session_id,
        item_id,
        quantity,
    };
    
    match send_request(request).await? {
        BuyerResponse::AddItemToCart => {
            println!("Item added to cart!");
            Ok(())
        }
        BuyerResponse::Error(msg) => {
            eprintln!("Error: {}", msg);
            Ok(())
        }
        _ => {
            eprintln!("Unexpected response");
            Ok(())
        }
    }
}

async fn remove_from_cart(
    session_id_str: String,
    item_id_str: String,
    quantity: i32,
) -> Result<(), Box<dyn std::error::Error>> {
    let session_id = Uuid::parse_str(&session_id_str)?;
    let item_id = Uuid::parse_str(&item_id_str)?;
    
    let request = BuyerRequest::RemoveItemFromCart {
        session_id,
        item_id,
        quantity,
    };
    
    match send_request(request).await? {
        BuyerResponse::RemoveItemFromCart => {
            println!("Item removed from cart!");
            Ok(())
        }
        BuyerResponse::Error(msg) => {
            eprintln!("Error: {}", msg);
            Ok(())
        }
        _ => {
            eprintln!("Unexpected response");
            Ok(())
        }
    }
}

async fn save_cart(session_id_str: String) -> Result<(), Box<dyn std::error::Error>> {
    let session_id = Uuid::parse_str(&session_id_str)?;
    
    let request = BuyerRequest::SaveCart { session_id };
    
    match send_request(request).await? {
        BuyerResponse::SaveCart => {
            println!("Cart saved!");
            Ok(())
        }
        BuyerResponse::Error(msg) => {
            eprintln!("Error: {}", msg);
            Ok(())
        }
        _ => {
            eprintln!("Unexpected response");
            Ok(())
        }
    }
}

async fn clear_cart(session_id_str: String) -> Result<(), Box<dyn std::error::Error>> {
    let session_id = Uuid::parse_str(&session_id_str)?;
    
    let request = BuyerRequest::ClearCart { session_id };
    
    match send_request(request).await? {
        BuyerResponse::ClearCart => {
            println!("Cart cleared!");
            Ok(())
        }
        BuyerResponse::Error(msg) => {
            eprintln!("Error: {}", msg);
            Ok(())
        }
        _ => {
            eprintln!("Unexpected response");
            Ok(())
        }
    }
}

async fn display_cart(session_id_str: String) -> Result<(), Box<dyn std::error::Error>> {
    let session_id = Uuid::parse_str(&session_id_str)?;
    
    let request = BuyerRequest::DisplayCart { session_id };
    
    match send_request(request).await? {
        BuyerResponse::DisplayCart(cart_items) => {
            if cart_items.is_empty() {
                println!("Cart is empty.");
                return Ok(());
            }
            
            println!("Shopping Cart:");
            println!("{:-<80}", "");
            let mut total = 0.0;
            for item in &cart_items {
                println!("Item ID: {}", item.item_id);
                println!("  Quantity: {}", item.quantity);
                // Note: To get price, we'd need to fetch item details
                total += item.quantity as f64; // Placeholder
                println!("{:-<80}", "");
            }
            println!("Total items: {}", cart_items.len());
            println!("Total quantity (placeholder): {}", total);
            Ok(())
        }
        BuyerResponse::Error(msg) => {
            eprintln!("Error: {}", msg);
            Ok(())
        }
        _ => {
            eprintln!("Unexpected response");
            Ok(())
        }
    }
}

async fn feedback(
    session_id_str: String,
    item_id_str: String,
    thumbs_up: bool,
) -> Result<(), Box<dyn std::error::Error>> {
    let session_id = Uuid::parse_str(&session_id_str)?;
    let item_id = Uuid::parse_str(&item_id_str)?;
    
    let request = BuyerRequest::ProvideFeedback {
        session_id,
        item_id,
        thumbs_up,
    };
    
    match send_request(request).await? {
        BuyerResponse::ProvideFeedback => {
            println!("Feedback submitted!");
            Ok(())
        }
        BuyerResponse::Error(msg) => {
            eprintln!("Error: {}", msg);
            Ok(())
        }
        _ => {
            eprintln!("Unexpected response");
            Ok(())
        }
    }
}

async fn get_seller_rating(
    session_id_str: String,
    seller_id_str: String,
) -> Result<(), Box<dyn std::error::Error>> {
    let session_id = Uuid::parse_str(&session_id_str)?;
    let seller_id = Uuid::parse_str(&seller_id_str)?;
    
    let request = BuyerRequest::GetSellerRating {
        session_id,
        seller_id,
    };
    
    match send_request(request).await? {
        BuyerResponse::GetSellerRating(feedback) => {
            println!("Seller Rating:");
            println!("  Thumbs Up: {}", feedback.thumbs_up);
            println!("  Thumbs Down: {}", feedback.thumbs_down);
            let total = feedback.thumbs_up + feedback.thumbs_down;
            if total > 0 {
                let rating = (feedback.thumbs_up as f64 / total as f64) * 100.0;
                println!("  Rating: {:.1}%", rating);
            }
            Ok(())
        }
        BuyerResponse::Error(msg) => {
            eprintln!("Error: {}", msg);
            Ok(())
        }
        _ => {
            eprintln!("Unexpected response");
            Ok(())
        }
    }
}

async fn get_purchases(session_id_str: String) -> Result<(), Box<dyn std::error::Error>> {
    let session_id = Uuid::parse_str(&session_id_str)?;
    
    let request = BuyerRequest::GetBuyerPurchases { session_id };
    
    match send_request(request).await? {
        BuyerResponse::GetBuyerPurchases(history) => {
            if history.is_empty() {
                println!("No purchase history.");
                return Ok(());
            }
            
            println!("Purchase History ({} items):", history.len());
            for (i, item_id) in history.iter().enumerate() {
                println!("{}. Item ID: {}", i + 1, item_id);
            }
            Ok(())
        }
        BuyerResponse::Error(msg) => {
            eprintln!("Error: {}", msg);
            Ok(())
        }
        _ => {
            eprintln!("Unexpected response");
            Ok(())
        }
    }
}