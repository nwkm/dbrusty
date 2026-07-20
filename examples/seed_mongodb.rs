use bson::{doc, Document};
/// Seed MongoDB with random fake data for testing.
///
/// Usage: cargo run --example seed_mongodb [uri] [database]
///   uri:      mongodb://127.0.0.1:27017 (default)
///   database: dbrusty_seed (default)
use mongodb::Client;
use rand::Rng;
use std::time::Instant;

fn pick<'a, T>(items: &'a [T]) -> &'a T {
    &items[rand::rng().random_range(0..items.len())]
}

fn random_name() -> String {
    let first = [
        "Alice", "Bob", "Charlie", "Diana", "Eve", "Frank", "Grace", "Hank", "Iris", "Jack",
        "Kate", "Liam", "Mia", "Noah", "Olivia", "Paul",
    ];
    let last = [
        "Smith",
        "Johnson",
        "Williams",
        "Brown",
        "Jones",
        "Garcia",
        "Miller",
        "Davis",
        "Rodriguez",
        "Martinez",
        "Hernandez",
        "Lopez",
        "Wilson",
    ];
    format!("{} {}", pick(&first), pick(&last))
}

fn random_email(name: &str) -> String {
    let domains = [
        "gmail.com",
        "yahoo.com",
        "outlook.com",
        "example.com",
        "acme.org",
    ];
    let local = name.to_lowercase().replace(' ', ".");
    format!("{}@{}", local, pick(&domains))
}

fn random_price() -> f64 {
    let mut rng = rand::rng();
    let v: f64 = rng.random_range(1.0..999.99);
    (v * 100.0).round() / 100.0
}

#[tokio::main]
async fn main() {
    let args: Vec<String> = std::env::args().collect();
    let uri = args
        .get(1)
        .cloned()
        .unwrap_or_else(|| "mongodb://127.0.0.1:27017".into());
    let db_name = args
        .get(2)
        .cloned()
        .unwrap_or_else(|| "dbrusty_seed".into());

    println!("Connecting to {} ...", uri);
    let client = Client::with_uri_str(&uri).await.expect("connect failed");
    let db = client.database(&db_name);

    let start = Instant::now();

    // ── users (100 docs) ────────────────────────────────────────────
    {
        let coll = db.collection::<Document>("users");
        let mut docs = Vec::with_capacity(100);
        for _ in 0..100 {
            let mut rng = rand::rng();
            let name = random_name();
            let tag1 = if rng.random_bool(0.3) {
                "premium"
            } else {
                "standard"
            };
            let tag2 = if rng.random_bool(0.1) {
                "vip"
            } else {
                "regular"
            };
            docs.push(doc! {
                "name": &name,
                "email": random_email(&name),
                "age": rng.random_range(18..80),
                "active": rng.random_bool(0.85),
                "score": rng.random_range(1.0..100.0) as f64,
                "created_at": bson::DateTime::now(),
                "tags": [tag1, tag2],
            });
        }
        coll.insert_many(docs).await.expect("insert users failed");
        println!("  ✓ users: 100 documents");
    }

    // ── products (50 docs) ──────────────────────────────────────────
    {
        let coll = db.collection::<Document>("products");
        let cats = [
            "Electronics",
            "Clothing",
            "Books",
            "Home",
            "Sports",
            "Food",
            "Toys",
        ];
        let mut docs = Vec::with_capacity(50);
        for i in 1..=50 {
            let mut rng = rand::rng();
            let base: f64 = rng.random_range(1.0..5.0);
            let rating: f64 = (base * 10.0).round() / 10.0;
            docs.push(doc! {
                "sku": format!("SKU-{:04}", i),
                "name": format!("Product {}", i),
                "category": pick(&cats),
                "price": random_price(),
                "rating": rating,
                "in_stock": rng.random_bool(0.75),
                "stock_count": rng.random_range(0..500),
                "created_at": bson::DateTime::now(),
            });
        }
        coll.insert_many(docs)
            .await
            .expect("insert products failed");
        println!("  ✓ products: 50 documents");
    }

    // ── orders (200 docs) ───────────────────────────────────────────
    {
        let coll = db.collection::<Document>("orders");
        let statuses = ["pending", "shipped", "delivered", "cancelled", "refunded"];
        let mut docs = Vec::with_capacity(200);
        for _ in 0..200 {
            let mut rng = rand::rng();
            let item_count = rng.random_range(1..6);
            let mut items = Vec::with_capacity(item_count);
            let mut total: f64 = 0.0;
            for _ in 0..item_count {
                let qty = rng.random_range(1..4);
                let price = random_price();
                total += price * qty as f64;
                items.push(doc! {
                    "product_id": format!("SKU-{:04}", rng.random_range(1..51)),
                    "qty": qty,
                    "price": price,
                });
            }
            let t: f64 = total;
            total = (t * 100.0).round() / 100.0;
            docs.push(doc! {
                "order_id": uuid::Uuid::new_v4().to_string(),
                "items": &items,
                "total": total,
                "status": pick(&statuses),
                "created_at": bson::DateTime::now(),
            });
        }
        coll.insert_many(docs).await.expect("insert orders failed");
        println!("  ✓ orders: 200 documents");
    }

    // ── logs (500 docs) ─────────────────────────────────────────────
    {
        let coll = db.collection::<Document>("logs");
        let levels = ["info", "warn", "error", "debug"];
        let services = ["api", "web", "db", "auth", "worker", "cron"];
        let mut docs = Vec::with_capacity(500);
        for _ in 0..500 {
            let mut rng = rand::rng();
            docs.push(doc! {
                "timestamp": bson::DateTime::now(),
                "level": pick(&levels),
                "service": pick(&services),
                "message": format!("log entry {}", rng.random_range(1..10000)),
                "duration_ms": rng.random_range(1..5000),
            });
        }
        coll.insert_many(docs).await.expect("insert logs failed");
        println!("  ✓ logs: 500 documents");
    }

    // ── analytics_events (1000 docs) ────────────────────────────────
    {
        let coll = db.collection::<Document>("analytics_events");
        let events = [
            "page_view",
            "click",
            "signup",
            "purchase",
            "login",
            "logout",
            "search",
        ];
        let browsers = ["Chrome", "Firefox", "Safari", "Edge"];
        let oss = ["MacOS", "Windows", "Linux", "iOS", "Android"];
        let refs = ["https://google.com", "https://twitter.com", "direct"];
        let mut docs = Vec::with_capacity(1000);
        for _ in 0..1000 {
            let mut rng = rand::rng();
            let res = format!(
                "{}x{}",
                rng.random_range(1024..3840),
                rng.random_range(768..2160)
            );
            let metadata = doc! {
                "browser": pick(&browsers),
                "os": pick(&oss),
                "screen": &res,
                "is_mobile": rng.random_bool(0.35),
                "referrer": pick(&refs),
            };
            docs.push(doc! {
                "event": pick(&events),
                "session_id": uuid::Uuid::new_v4().to_string(),
                "user_id": rng.random_range(1..1001),
                "metadata": metadata,
                "timestamp": bson::DateTime::now(),
            });
        }
        coll.insert_many(docs)
            .await
            .expect("insert analytics failed");
        println!("  ✓ analytics_events: 1000 documents");
    }

    // ── companies (nested departments > employees) ─────────────────
    {
        let coll = db.collection::<Document>("companies");
        let dept_names = [
            "Engineering",
            "Sales",
            "Marketing",
            "HR",
            "Finance",
            "Support",
        ];
        let titles = ["Junior", "Senior", "Lead", "Manager", "Director"];
        let skills_pool = [
            "Rust",
            "Python",
            "JavaScript",
            "Go",
            "SQL",
            "MongoDB",
            "Kubernetes",
        ];
        let mut docs = Vec::with_capacity(10);
        for c in 1..=10 {
            let mut rng = rand::rng();
            let dept_count = rng.random_range(2..5);
            let mut departments = Vec::with_capacity(dept_count);
            for d in 0..dept_count {
                let emp_count = rng.random_range(3..8);
                let mut employees = Vec::with_capacity(emp_count);
                for _ in 0..emp_count {
                    let name = random_name();
                    employees.push(doc! {
                        "name": &name,
                        "email": random_email(&name),
                        "title": pick(&titles),
                        "salary": rng.random_range(40000..200000),
                        "start_date": bson::DateTime::now(),
                        "skills": [pick(&skills_pool)],
                    });
                }
                departments.push(doc! {
                    "name": dept_names[d % dept_names.len()],
                    "budget": rng.random_range(100000..1_000_000),
                    "headcount": emp_count as i32,
                    "employees": &employees,
                });
            }
            docs.push(doc! {
                "name": format!("Company {}", c),
                "domain": format!("company{}.com", c),
                "founded": rng.random_range(1990..2024),
                "revenue": rng.random_range(1_000_000..100_000_000),
                "departments": &departments,
            });
        }
        coll.insert_many(docs)
            .await
            .expect("insert companies failed");
        println!("  ✓ companies: 10 documents (nested)");
    }

    let elapsed = start.elapsed();
    println!("\nSeeded database '{}' in {:?}", db_name, elapsed);
    println!("Collections: users (100), products (50), orders (200), logs (500), analytics_events (1000), companies (10)");
}
