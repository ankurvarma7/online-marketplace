use clap::{Parser, Subcommand};
use common::*;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use uuid::Uuid;

fn get_seller_server_addr() -> String {
    std::env::var("SELLER_SERVER_ADDR").unwrap_or_else(|_| "127.0.0.1:8082".to_string())
}

#[derive(Parser)]
#[command(name = "seller_client")]
#[command(about = "Online Marketplace Seller Client")]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Create a new seller account
    CreateAccount {
        #[arg(short, long)]
        name: String,
        #[arg(short, long)]
        password: String,
    },
    /// Login to seller account
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
    /// Get seller rating
    GetRating {
        #[arg(short, long)]
        session_id: String,
    },
    /// Register an item for sale
    RegisterItem {
        #[arg(short, long)]
        session_id: String,
        #[arg(short, long)]
        name: String,
        #[arg(short, long)]
        category: i32,
        #[arg(short, long, num_args = 1..=5, value_delimiter = ',')]
        keywords: Vec<String>,
        #[arg(short, long)]
        condition: String,
        #[arg(short, long)]
        price: f64,
        #[arg(short, long)]
        quantity: i32,
    },
    /// Change item price
    ChangePrice {
        #[arg(short, long)]
        session_id: String,
        #[arg(short, long)]
        item_id: String,
        #[arg(short, long)]
        new_price: f64,
    },
    /// Update units for sale
    UpdateUnits {
        #[arg(short, long)]
        session_id: String,
        #[arg(short, long)]
        item_id: String,
        #[arg(short, long)]
        quantity: i32,
    },
    /// Display items for sale
    DisplayItems {
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
        Commands::GetRating { session_id } => {
            get_rating(session_id).await?;
        }
        Commands::RegisterItem {
            session_id,
            name,
            category,
            keywords,
            condition,
            price,
            quantity,
        } => {
            register_item(
                session_id,
                name,
                category,
                keywords,
                condition,
                price,
                quantity,
            ).await?;
        }
        Commands::ChangePrice {
            session_id,
            item_id,
            new_price,
        } => {
            change_price(session_id, item_id, new_price).await?;
        }
        Commands::UpdateUnits {
            session_id,
            item_id,
            quantity,
        } => {
            update_units(session_id, item_id, quantity).await?;
        }
        Commands::DisplayItems { session_id } => {
            display_items(session_id).await?;
        }
    }
    
    Ok(())
}

async fn send_request(request: SellerRequest) -> Result<SellerResponse, Box<dyn std::error::Error>> {
    let addr = get_seller_server_addr();
    let mut stream = tokio::net::TcpStream::connect(&addr).await?;
    
    let request_str = serde_json::to_string(&request)?;
    stream.write_all(request_str.as_bytes()).await?;
    stream.write_all(b"\n").await?;
    
    let mut response_str = String::new();
    let mut reader = BufReader::new(stream);
    reader.read_line(&mut response_str).await?;
    
    let response: SellerResponse = serde_json::from_str(&response_str)?;
    Ok(response)
}

async fn create_account(name: String, password: String) -> Result<(), Box<dyn std::error::Error>> {
    let request = SellerRequest::CreateAccount {
        seller_name: name,
        password,
    };
    
    match send_request(request).await? {
        SellerResponse::CreateAccount(seller_id) => {
            println!("Account created successfully!");
            println!("Seller ID: {}", seller_id);
            Ok(())
        }
        SellerResponse::Error(msg) => {
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
    let request = SellerRequest::Login {
        seller_name: name,
        password,
    };
    
    match send_request(request).await? {
        SellerResponse::Login(session_id) => {
            println!("Login successful!");
            println!("Session ID: {}", session_id);
            println!("Session expires in 5 minutes");
            Ok(())
        }
        SellerResponse::Error(msg) => {
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
    
    let request = SellerRequest::Logout { session_id };
    
    match send_request(request).await? {
        SellerResponse::Logout => {
            println!("Logout successful!");
            Ok(())
        }
        SellerResponse::Error(msg) => {
            eprintln!("Error: {}", msg);
            Ok(())
        }
        _ => {
            eprintln!("Unexpected response");
            Ok(())
        }
    }
}

async fn get_rating(session_id_str: String) -> Result<(), Box<dyn std::error::Error>> {
    let session_id = Uuid::parse_str(&session_id_str)?;
    
    let request = SellerRequest::GetSellerRating { session_id };
    
    match send_request(request).await? {
        SellerResponse::GetSellerRating(feedback) => {
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
        SellerResponse::Error(msg) => {
            eprintln!("Error: {}", msg);
            Ok(())
        }
        _ => {
            eprintln!("Unexpected response");
            Ok(())
        }
    }
}

async fn register_item(
    session_id_str: String,
    name: String,
    category: i32,
    keywords: Vec<String>,
    condition_str: String,
    price: f64,
    quantity: i32,
) -> Result<(), Box<dyn std::error::Error>> {
    let session_id = Uuid::parse_str(&session_id_str)?;
    
    let condition = match condition_str.to_lowercase().as_str() {
        "new" => Condition::New,
        "used" => Condition::Used,
        _ => return Err("Condition must be 'new' or 'used'".into()),
    };
    
    // Validate keywords
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
    
    let request = SellerRequest::RegisterItemForSale {
        session_id,
        item_name: name,
        item_category: category,
        keywords,
        condition,
        sale_price: price,
        quantity,
    };
    
    match send_request(request).await? {
        SellerResponse::RegisterItemForSale(item_id) => {
            println!("Item registered successfully!");
            println!("Item ID: {}", item_id);
            Ok(())
        }
        SellerResponse::Error(msg) => {
            eprintln!("Error: {}", msg);
            Ok(())
        }
        _ => {
            eprintln!("Unexpected response");
            Ok(())
        }
    }
}

async fn change_price(
    session_id_str: String,
    item_id_str: String,
    new_price: f64,
) -> Result<(), Box<dyn std::error::Error>> {
    let session_id = Uuid::parse_str(&session_id_str)?;
    let item_id = Uuid::parse_str(&item_id_str)?;
    
    let request = SellerRequest::ChangeItemPrice {
        session_id,
        item_id,
        new_price,
    };
    
    match send_request(request).await? {
        SellerResponse::ChangeItemPrice => {
            println!("Price changed successfully!");
            Ok(())
        }
        SellerResponse::Error(msg) => {
            eprintln!("Error: {}", msg);
            Ok(())
        }
        _ => {
            eprintln!("Unexpected response");
            Ok(())
        }
    }
}

async fn update_units(
    session_id_str: String,
    item_id_str: String,
    quantity: i32,
) -> Result<(), Box<dyn std::error::Error>> {
    let session_id = Uuid::parse_str(&session_id_str)?;
    let item_id = Uuid::parse_str(&item_id_str)?;
    
    let request = SellerRequest::UpdateUnitsForSale {
        session_id,
        item_id,
        quantity,
    };
    
    match send_request(request).await? {
        SellerResponse::UpdateUnitsForSale => {
            println!("Units updated successfully!");
            Ok(())
        }
        SellerResponse::Error(msg) => {
            eprintln!("Error: {}", msg);
            Ok(())
        }
        _ => {
            eprintln!("Unexpected response");
            Ok(())
        }
    }
}

async fn display_items(session_id_str: String) -> Result<(), Box<dyn std::error::Error>> {
    let session_id = Uuid::parse_str(&session_id_str)?;
    
    let request = SellerRequest::DisplayItemsForSale { session_id };
    
    match send_request(request).await? {
        SellerResponse::DisplayItemsForSale(items) => {
            if items.is_empty() {
                println!("No items for sale.");
                return Ok(());
            }
            
            println!("Your Items for Sale:");
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
        SellerResponse::Error(msg) => {
            eprintln!("Error: {}", msg);
            Ok(())
        }
        _ => {
            eprintln!("Unexpected response");
            Ok(())
        }
    }
}