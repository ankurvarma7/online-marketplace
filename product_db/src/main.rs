use common::*;
use dashmap::DashMap;
use std::sync::Arc;
use tokio::net::{TcpListener, TcpStream};
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use uuid::Uuid;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let bind_addr = std::env::var("PRODUCT_DB_BIND_ADDR").unwrap_or_else(|_| "127.0.0.1:8081".to_string());
    let listener = TcpListener::bind(&bind_addr).await?;
    println!("Product Database listening on {}", bind_addr);
    
    // In-memory storage
    let items: Arc<DashMap<Uuid, Item>> = Arc::new(DashMap::new());
    let carts: Arc<DashMap<Uuid, Vec<CartItem>>> = Arc::new(DashMap::new());
    let purchase_history: Arc<DashMap<Uuid, Vec<Uuid>>> = Arc::new(DashMap::new());
    
    // Indexes for faster search
    let seller_items: Arc<DashMap<Uuid, Vec<Uuid>>> = Arc::new(DashMap::new());
    let category_items: Arc<DashMap<i32, Vec<Uuid>>> = Arc::new(DashMap::new());
    
    loop {
        let (socket, _) = listener.accept().await?;
        let items_clone = items.clone();
        let carts_clone = carts.clone();
        let purchase_history_clone = purchase_history.clone();
        let seller_items_clone = seller_items.clone();
        let category_items_clone = category_items.clone();
        
        tokio::spawn(async move {
            handle_connection(
                socket,
                items_clone,
                carts_clone,
                purchase_history_clone,
                seller_items_clone,
                category_items_clone,
            ).await;
        });
    }
}

async fn handle_connection(
    socket: TcpStream,
    items: Arc<DashMap<Uuid, Item>>,
    carts: Arc<DashMap<Uuid, Vec<CartItem>>>,
    purchase_history: Arc<DashMap<Uuid, Vec<Uuid>>>,
    seller_items: Arc<DashMap<Uuid, Vec<Uuid>>>,
    category_items: Arc<DashMap<i32, Vec<Uuid>>>,
) {
    let (read_half, mut write_half) = socket.into_split();
    let reader = BufReader::new(read_half);
    let mut lines = reader.lines();
    
    while let Ok(Some(line)) = lines.next_line().await {
        let request: ProductDbRequest = match serde_json::from_str(&line) {
            Ok(req) => req,
            Err(e) => {
                let response = ProductDbResponse::Error(format!("Invalid request: {}", e));
                let _ = send_response(&mut write_half, response).await;
                continue;
            }
        };
        
        let response = handle_request(
            request,
            &items,
            &carts,
            &purchase_history,
            &seller_items,
            &category_items,
        ).await;
        
        let _ = send_response(&mut write_half, response).await;
    }
}

async fn handle_request(
    request: ProductDbRequest,
    items: &DashMap<Uuid, Item>,
    carts: &DashMap<Uuid, Vec<CartItem>>,
    purchase_history: &DashMap<Uuid, Vec<Uuid>>,
    seller_items: &DashMap<Uuid, Vec<Uuid>>,
    category_items: &DashMap<i32, Vec<Uuid>>,
) -> ProductDbResponse {
    match request {
        ProductDbRequest::CreateItem { mut item } => {
            let item_id = Uuid::new_v4();
            item.item_id = item_id;
            
            // Insert item
            items.insert(item_id, item.clone());
            
            // Update indexes
            seller_items.entry(item.seller_id)
                .or_insert_with(Vec::new)
                .push(item_id);
            
            category_items.entry(item.item_category)
                .or_insert_with(Vec::new)
                .push(item_id);
            
            ProductDbResponse::ItemCreated(item_id)
        }
        
        ProductDbRequest::UpdateItem { item } => {
            items.insert(item.item_id, item.clone());
            ProductDbResponse::ItemUpdated
        }
        
        ProductDbRequest::GetItem { item_id } => {
            let item = items.get(&item_id).map(|i| i.clone());
            ProductDbResponse::Item(item)
        }
        
        ProductDbRequest::GetItemsBySeller { seller_id } => {
            let seller_items_list = seller_items.get(&seller_id)
                .map(|list| list.clone())
                .unwrap_or_default();
            
            let mut items_list = Vec::new();
            for item_id in seller_items_list {
                if let Some(item) = items.get(&item_id) {
                    items_list.push(item.clone());
                }
            }
            
            ProductDbResponse::Items(items_list)
        }
        
        ProductDbRequest::SearchItems { category, keywords } => {
            let mut results = Vec::new();
            
            // If category is specified, use category index
            if let Some(cat) = category {
                if let Some(item_ids) = category_items.get(&cat) {
                    for item_id in item_ids.iter() {
                        if let Some(item) = items.get(item_id) {
                            // Check keywords if provided
                            if keywords.is_empty() || 
                               keywords.iter().all(|kw| item.keywords.contains(kw)) {
                                results.push(item.clone());
                            }
                        }
                    }
                }
            } else {
                // Search all items
                for item in items.iter() {
                    if keywords.is_empty() || 
                       keywords.iter().all(|kw| item.keywords.contains(kw)) {
                        results.push(item.clone());
                    }
                }
            }
            
            // Sort by best match (simple implementation)
            results.sort_by(|a, b| {
                let a_matches = keywords.iter()
                    .filter(|kw| a.keywords.contains(kw))
                    .count();
                let b_matches = keywords.iter()
                    .filter(|kw| b.keywords.contains(kw))
                    .count();
                b_matches.cmp(&a_matches)
            });
            
            ProductDbResponse::Items(results)
        }
        
        ProductDbRequest::AddToCart { buyer_id, item_id, quantity } => {
            if let Some(item) = items.get(&item_id) {
                if item.quantity < quantity {
                    return ProductDbResponse::Error("Insufficient quantity".to_string());
                }
                
                let mut cart = carts.entry(buyer_id).or_insert_with(Vec::new);
                
                if let Some(cart_item) = cart.iter_mut().find(|ci| ci.item_id == item_id) {
                    cart_item.quantity += quantity;
                } else {
                    cart.push(CartItem { item_id, quantity });
                }
                
                ProductDbResponse::CartSaved
            } else {
                ProductDbResponse::Error("Item not found".to_string())
            }
        }
        
        ProductDbRequest::RemoveFromCart { buyer_id, item_id, quantity } => {
            if let Some(mut cart) = carts.get_mut(&buyer_id) {
                if let Some(index) = cart.iter().position(|ci| ci.item_id == item_id) {
                    if cart[index].quantity <= quantity {
                        cart.remove(index);
                    } else {
                        cart[index].quantity -= quantity;
                    }
                }
            }
            
            ProductDbResponse::CartSaved
        }
        
        ProductDbRequest::GetCart { buyer_id } => {
            let cart = carts.get(&buyer_id)
                .map(|c| c.clone())
                .unwrap_or_default();
            ProductDbResponse::Cart(cart)
        }
        
        ProductDbRequest::SaveCart { buyer_id, cart } => {
            carts.insert(buyer_id, cart);
            ProductDbResponse::CartSaved
        }
        
        ProductDbRequest::ClearCart { buyer_id } => {
            carts.remove(&buyer_id);
            ProductDbResponse::CartCleared
        }
        
        ProductDbRequest::AddPurchaseHistory { buyer_id, item_id } => {
            purchase_history.entry(buyer_id)
                .or_insert_with(Vec::new)
                .push(item_id);
            ProductDbResponse::PurchaseRecorded
        }
        
        ProductDbRequest::GetPurchaseHistory { buyer_id } => {
            let history = purchase_history.get(&buyer_id)
                .map(|h| h.clone())
                .unwrap_or_default();
            ProductDbResponse::PurchaseHistory(history)
        }
    }
}

async fn send_response(
    writer: &mut tokio::net::tcp::OwnedWriteHalf,
    response: ProductDbResponse,
) -> Result<(), Box<dyn std::error::Error>> {
    let response_str = serde_json::to_string(&response)?;
    writer.write_all(response_str.as_bytes()).await?;
    writer.write_all(b"\n").await?;
    Ok(())
}