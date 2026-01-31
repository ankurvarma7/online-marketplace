use serde::{Deserialize, Serialize};
use uuid::Uuid;

// Shared data structures

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Item {
    pub item_id: Uuid,
    pub item_name: String,
    pub item_category: i32,
    pub keywords: Vec<String>,
    pub condition: Condition,
    pub sale_price: f64,
    pub quantity: i32,
    pub feedback: Feedback,
    pub seller_id: Uuid,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub enum Condition {
    New,
    Used,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Feedback {
    pub thumbs_up: i32,
    pub thumbs_down: i32,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Seller {
    pub seller_id: Uuid,
    pub seller_name: String,
    pub feedback: Feedback,
    pub items_sold: i32,
    pub password: String, // In production, this should be hashed
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Buyer {
    pub buyer_id: Uuid,
    pub buyer_name: String,
    pub items_purchased: i32,
    pub password: String,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Session {
    pub session_id: Uuid,
    pub user_id: Uuid,
    pub user_type: UserType,
    pub expiration: i64,
}

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq)]
pub enum UserType {
    Buyer,
    Seller,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct CartItem {
    pub item_id: Uuid,
    pub quantity: i32,
}

// Message types for TCP communication

#[derive(Debug, Serialize, Deserialize)]
pub enum SellerRequest {
    CreateAccount {
        seller_name: String,
        password: String,
    },
    Login {
        seller_name: String,
        password: String,
    },
    Logout {
        session_id: Uuid,
    },
    GetSellerRating {
        session_id: Uuid,
    },
    RegisterItemForSale {
        session_id: Uuid,
        item_name: String,
        item_category: i32,
        keywords: Vec<String>,
        condition: Condition,
        sale_price: f64,
        quantity: i32,
    },
    ChangeItemPrice {
        session_id: Uuid,
        item_id: Uuid,
        new_price: f64,
    },
    UpdateUnitsForSale {
        session_id: Uuid,
        item_id: Uuid,
        quantity: i32,
    },
    DisplayItemsForSale {
        session_id: Uuid,
    },
}

#[derive(Debug, Serialize, Deserialize)]
pub enum SellerResponse {
    CreateAccount(Uuid),
    Login(Uuid),
    Logout,
    GetSellerRating(Feedback),
    RegisterItemForSale(Uuid),
    ChangeItemPrice,
    UpdateUnitsForSale,
    DisplayItemsForSale(Vec<Item>),
    Error(String),
}

#[derive(Debug, Serialize, Deserialize)]
pub enum BuyerRequest {
    CreateAccount {
        buyer_name: String,
        password: String,
    },
    Login {
        buyer_name: String,
        password: String,
    },
    Logout {
        session_id: Uuid,
    },
    SearchItemsForSale {
        session_id: Uuid,
        category: Option<i32>,
        keywords: Vec<String>,
    },
    GetItem {
        session_id: Uuid,
        item_id: Uuid,
    },
    AddItemToCart {
        session_id: Uuid,
        item_id: Uuid,
        quantity: i32,
    },
    RemoveItemFromCart {
        session_id: Uuid,
        item_id: Uuid,
        quantity: i32,
    },
    SaveCart {
        session_id: Uuid,
    },
    ClearCart {
        session_id: Uuid,
    },
    DisplayCart {
        session_id: Uuid,
    },
    ProvideFeedback {
        session_id: Uuid,
        item_id: Uuid,
        thumbs_up: bool,
    },
    GetSellerRating {
        session_id: Uuid,
        seller_id: Uuid,
    },
    GetBuyerPurchases {
        session_id: Uuid,
    },
}

#[derive(Debug, Serialize, Deserialize)]
pub enum BuyerResponse {
    CreateAccount(Uuid),
    Login(Uuid),
    Logout,
    SearchItemsForSale(Vec<Item>),
    GetItem(Option<Item>),
    AddItemToCart,
    RemoveItemFromCart,
    SaveCart,
    ClearCart,
    DisplayCart(Vec<CartItem>),
    ProvideFeedback,
    GetSellerRating(Feedback),
    GetBuyerPurchases(Vec<Uuid>),
    Error(String),
}

// Database request/response types

#[derive(Debug, Serialize, Deserialize)]
pub enum CustomerDbRequest {
    CreateSeller {
        seller_name: String,
        password: String,
    },
    CreateBuyer {
        buyer_name: String,
        password: String,
    },
    GetSellerByName {
        seller_name: String,
    },
    GetBuyerByName {
        buyer_name: String,
    },
    GetSeller {
        seller_id: Uuid,
    },
    UpdateSeller {
        seller: Seller,
    },
    GetBuyer {
        buyer_id: Uuid,
    },
    UpdateBuyer {
        buyer: Buyer,
    },
    CreateSession {
        user_id: Uuid,
        user_type: UserType,
    },
    GetSession {
        session_id: Uuid,
    },
    DeleteSession {
        session_id: Uuid,
    },
    CleanupSessions,
}

#[derive(Debug, Serialize, Deserialize)]
pub enum CustomerDbResponse {
    SellerCreated(Uuid),
    BuyerCreated(Uuid),
    Seller(Option<Seller>),
    Buyer(Option<Buyer>),
    SellerUpdated,
    BuyerUpdated,
    SessionCreated(Uuid, i64),
    Session(Option<Session>),
    SessionDeleted,
    SessionsCleaned(usize),
    Error(String),
}

#[derive(Debug, Serialize, Deserialize)]
pub enum ProductDbRequest {
    CreateItem {
        item: Item,
    },
    UpdateItem {
        item: Item,
    },
    GetItem {
        item_id: Uuid,
    },
    GetItemsBySeller {
        seller_id: Uuid,
    },
    SearchItems {
        category: Option<i32>,
        keywords: Vec<String>,
    },
    AddToCart {
        buyer_id: Uuid,
        item_id: Uuid,
        quantity: i32,
    },
    RemoveFromCart {
        buyer_id: Uuid,
        item_id: Uuid,
        quantity: i32,
    },
    GetCart {
        buyer_id: Uuid,
    },
    SaveCart {
        buyer_id: Uuid,
        cart: Vec<CartItem>,
    },
    ClearCart {
        buyer_id: Uuid,
    },
    AddPurchaseHistory {
        buyer_id: Uuid,
        item_id: Uuid,
    },
    GetPurchaseHistory {
        buyer_id: Uuid,
    },
}

#[derive(Debug, Serialize, Deserialize)]
pub enum ProductDbResponse {
    ItemCreated(Uuid),
    ItemUpdated,
    Item(Option<Item>),
    Items(Vec<Item>),
    Cart(Vec<CartItem>),
    CartSaved,
    CartCleared,
    PurchaseHistory(Vec<Uuid>),
    PurchaseRecorded,
    Error(String),
}