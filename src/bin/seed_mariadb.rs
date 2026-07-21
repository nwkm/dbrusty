use std::time::Instant;

use rand::Rng;
use sqlx::mysql::MySqlPoolOptions;
use sqlx::MySqlPool;

const BATCH_SIZE: usize = 100;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<String> = std::env::args().collect();
    let uri = if args.len() > 1 {
        &args[1]
    } else {
        "mysql://root:password@localhost:3306/mariadb"
    };

    println!("Connecting to: {}", uri);
    println!("Note: The database named in the URI must already exist.");
    let pool = MySqlPoolOptions::new()
        .max_connections(5)
        .connect(uri)
        .await?;

    let start = Instant::now();
    create_tables(&pool).await?;

    let mut total: usize = 0;
    total += seed_categories(&pool).await?;
    total += seed_users(&pool).await?;
    total += seed_products(&pool).await?;
    total += seed_tags(&pool).await?;
    total += seed_product_tags(&pool).await?;
    total += seed_orders(&pool).await?;
    total += seed_order_items(&pool).await?;
    total += seed_reviews(&pool).await?;
    total += seed_events(&pool).await?;
    total += seed_logs(&pool).await?;
    total += seed_audit_trail(&pool).await?;
    total += seed_addresses(&pool).await?;

    println!(
        "\nDone — seeded {} rows across 12 tables in {:?}",
        total,
        start.elapsed()
    );
    Ok(())
}

async fn create_tables(pool: &MySqlPool) -> Result<(), sqlx::Error> {
    let sql = r#"
        CREATE TABLE IF NOT EXISTS seed_categories (
            id INT AUTO_INCREMENT PRIMARY KEY,
            name VARCHAR(100) NOT NULL,
            description TEXT,
            created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP
        );
        CREATE TABLE IF NOT EXISTS seed_users (
            id INT AUTO_INCREMENT PRIMARY KEY,
            name VARCHAR(100) NOT NULL,
            email VARCHAR(200) NOT NULL UNIQUE,
            age INT,
            is_active TINYINT(1) DEFAULT 1,
            salary DECIMAL(12,2),
            created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP
        );
        CREATE TABLE IF NOT EXISTS seed_products (
            id INT AUTO_INCREMENT PRIMARY KEY,
            name VARCHAR(200) NOT NULL,
            category_id INT REFERENCES seed_categories(id),
            price DECIMAL(10,2) NOT NULL,
            in_stock INT DEFAULT 0,
            weight_kg DECIMAL(6,2),
            description TEXT,
            created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP
        );
        CREATE TABLE IF NOT EXISTS seed_orders (
            id INT AUTO_INCREMENT PRIMARY KEY,
            user_id INT NOT NULL REFERENCES seed_users(id),
            total DECIMAL(12,2) NOT NULL,
            status VARCHAR(20) DEFAULT 'pending',
            shipping_address TEXT,
            created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP
        );
        CREATE TABLE IF NOT EXISTS seed_order_items (
            id INT AUTO_INCREMENT PRIMARY KEY,
            order_id INT NOT NULL REFERENCES seed_orders(id),
            product_id INT NOT NULL REFERENCES seed_products(id),
            quantity INT NOT NULL,
            unit_price DECIMAL(10,2) NOT NULL
        );
        CREATE TABLE IF NOT EXISTS seed_reviews (
            id INT AUTO_INCREMENT PRIMARY KEY,
            product_id INT NOT NULL REFERENCES seed_products(id),
            user_id INT NOT NULL REFERENCES seed_users(id),
            rating TINYINT(1),
            comment TEXT,
            created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP
        );
        CREATE TABLE IF NOT EXISTS seed_events (
            id INT AUTO_INCREMENT PRIMARY KEY,
            event_type VARCHAR(50) NOT NULL,
            payload JSON,
            created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP
        );
        CREATE TABLE IF NOT EXISTS seed_tags (
            id INT AUTO_INCREMENT PRIMARY KEY,
            name VARCHAR(50) NOT NULL,
            color VARCHAR(7)
        );
        CREATE TABLE IF NOT EXISTS seed_product_tags (
            product_id INT NOT NULL REFERENCES seed_products(id),
            tag_id INT NOT NULL REFERENCES seed_tags(id),
            PRIMARY KEY (product_id, tag_id)
        );
        CREATE TABLE IF NOT EXISTS seed_logs (
            id INT AUTO_INCREMENT PRIMARY KEY,
            level VARCHAR(10) NOT NULL,
            message TEXT NOT NULL,
            source VARCHAR(100),
            created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP
        );
        CREATE TABLE IF NOT EXISTS seed_audit_trail (
            id INT AUTO_INCREMENT PRIMARY KEY,
            table_name VARCHAR(100) NOT NULL,
            operation VARCHAR(10) NOT NULL,
            record_id INT,
            old_data JSON,
            new_data JSON,
            changed_by VARCHAR(100),
            changed_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP
        );
        CREATE TABLE IF NOT EXISTS seed_addresses (
            id INT AUTO_INCREMENT PRIMARY KEY,
            user_id INT NOT NULL REFERENCES seed_users(id),
            street VARCHAR(200),
            city VARCHAR(100),
            state VARCHAR(50),
            zip VARCHAR(20),
            country VARCHAR(100) DEFAULT 'USA',
            is_primary TINYINT(1) DEFAULT 0
        );
    "#;

    for statement in sql.split(';') {
        let stmt = statement.trim();
        if !stmt.is_empty() {
            sqlx::query(stmt).execute(pool).await?;
        }
    }
    println!("Tables created");
    Ok(())
}

fn random_name(rng: &mut impl Rng) -> String {
    let first = [
        "Alice", "Bob", "Charlie", "Diana", "Edward", "Fiona", "George", "Hannah", "Ivan",
        "Julia", "Kevin", "Laura", "Michael", "Nina", "Oscar", "Patricia", "Quinn", "Rachel",
        "Samuel", "Tina", "Uma", "Victor", "Wendy", "Xavier", "Yara", "Zach", "Aiden",
        "Bella", "Carter", "Daisy", "Ethan", "Freya", "Gavin", "Hazel", "Isaac", "Jade",
        "Kai", "Luna", "Mason", "Nora", "Owen", "Piper", "Riley", "Sage", "Theo",
        "Violet", "Wyatt", "Zoe", "Amir", "Blake", "Cora", "Dean", "Eliza", "Finn",
        "Gemma", "Hugo", "Iris", "Jasper", "Kira", "Leo", "Maya", "Nash", "Olive",
    ];
    let last = [
        "Smith", "Johnson", "Williams", "Brown", "Jones", "Garcia", "Miller", "Davis",
        "Rodriguez", "Martinez", "Hernandez", "Lopez", "Gonzalez", "Wilson", "Anderson",
        "Thomas", "Taylor", "Moore", "Jackson", "Martin", "Lee", "Perez", "Thompson",
        "White", "Harris", "Sanchez", "Clark", "Ramirez", "Lewis", "Robinson", "Walker",
        "Young", "Allen", "King", "Wright", "Scott", "Torres", "Nguyen", "Hill",
        "Adams", "Baker", "Green", "Campbell", "Mitchell", "Carter", "Roberts",
    ];
    format!(
        "{} {}",
        first[rng.random_range(0..first.len())],
        last[rng.random_range(0..last.len())]
    )
}

fn random_email(rng: &mut impl Rng, name: &str) -> String {
    let domains = [
        "example.com", "test.org", "demo.net", "mail.com", "inbox.co",
    ];
    let base = name.to_lowercase().replace(' ', ".");
    format!(
        "{}.{}@{}",
        base,
        rng.random_range(100..999),
        domains[rng.random_range(0..domains.len())]
    )
}

fn random_sentence(rng: &mut impl Rng, min_words: usize, max_words: usize) -> String {
    let words = [
        "lorem", "ipsum", "dolor", "sit", "amet", "consectetur", "adipiscing", "elit", "sed",
        "do", "eiusmod", "tempor", "incididunt", "ut", "labore", "et", "dolore", "magna",
        "aliqua", "enim", "ad", "minim", "veniam", "quis", "nostrud", "exercitation", "ullamco",
        "laboris", "nisi", "aliquip", "ex", "ea", "commodo", "consequat", "duis", "aute",
        "irure", "reprehenderit", "voluptate", "velit", "esse", "cillum", "fugiat", "nulla",
        "pariatur", "excepteur", "sint", "occaecat", "cupidatat", "non", "proident", "sunt",
        "culpa", "qui", "officia", "deserunt", "mollit", "anim", "id", "est", "laborum",
    ];
    let count = rng.random_range(min_words..=max_words);
    let mut s = String::new();
    for i in 0..count {
        if i > 0 {
            s.push(' ');
        }
        s.push_str(words[rng.random_range(0..words.len())]);
    }
    s.push('.');
    s
}

fn random_city(rng: &mut impl Rng) -> String {
    let cities = [
        "New York", "Los Angeles", "Chicago", "Houston", "Phoenix", "Philadelphia",
        "San Antonio", "San Diego", "Dallas", "Austin", "Portland", "Seattle", "Denver",
        "Boston", "Miami", "Atlanta", "Minneapolis", "Tampa", "Orlando", "Detroit",
    ];
    cities[rng.random_range(0..cities.len())].to_string()
}

fn random_state(rng: &mut impl Rng) -> String {
    let states = [
        "NY", "CA", "IL", "TX", "AZ", "PA", "FL", "OH", "GA", "NC", "MI", "WA", "CO",
        "MA", "OR", "MN", "TN", "IN", "MO", "WI",
    ];
    states[rng.random_range(0..states.len())].to_string()
}

fn random_zip(rng: &mut impl Rng) -> String {
    format!("{:05}", rng.random_range(10000..99999))
}

// ── Category data ──────────────────────────────────────────────────

const CATEGORIES: &[(&str, &str)] = &[
    ("Electronics", "Devices, gadgets, and electronic accessories"),
    ("Clothing", "Apparel, footwear, and fashion accessories"),
    ("Books", "Fiction, non-fiction, and educational materials"),
    ("Home & Garden", "Furniture, decor, and gardening supplies"),
    ("Sports", "Equipment, apparel, and gear for sports"),
    ("Food & Beverage", "Groceries, snacks, and beverages"),
    ("Health & Beauty", "Skincare, cosmetics, and wellness products"),
    ("Toys", "Games, puzzles, and children's toys"),
    ("Automotive", "Car parts, accessories, and maintenance"),
    ("Office", "Stationery, furniture, and office supplies"),
];

async fn seed_categories(pool: &MySqlPool) -> Result<usize, sqlx::Error> {
    println!("  Seeding seed_categories ...");
    for (name, desc) in CATEGORIES {
        sqlx::query(
            "INSERT IGNORE INTO seed_categories (name, description) VALUES (?, ?)",
        )
        .bind(name)
        .bind(desc)
        .execute(pool)
        .await?;
    }
    Ok(CATEGORIES.len())
}

async fn seed_users(pool: &MySqlPool) -> Result<usize, sqlx::Error> {
    let count = 100;
    println!("  Seeding {} seed_users ...", count);
    let mut rng = rand::rng();

    for chunk in 0..(count + BATCH_SIZE - 1) / BATCH_SIZE {
        let batch_size = BATCH_SIZE.min(count - chunk * BATCH_SIZE);

        let mut names: Vec<String> = Vec::with_capacity(batch_size);
        let mut emails: Vec<String> = Vec::with_capacity(batch_size);
        let mut ages: Vec<i32> = Vec::with_capacity(batch_size);
        let mut active_flags: Vec<i32> = Vec::with_capacity(batch_size);
        let mut salaries: Vec<f64> = Vec::with_capacity(batch_size);

        for _ in 0..batch_size {
            let name = random_name(&mut rng);
            let email = loop {
                let e = random_email(&mut rng, &name);
                if !emails.contains(&e) || names.is_empty() {
                    break e;
                }
            };
            names.push(name);
            emails.push(email);
            ages.push(rng.random_range(18..80));
            active_flags.push(if rng.random_bool(0.85) { 1 } else { 0 });
            let salary: f64 = rng.random_range(30_000.0..150_000.0);
            salaries.push((salary * 100.0).round() / 100.0);
        }

        let mut tx = pool.begin().await?;
        for i in 0..batch_size {
            sqlx::query(
                "INSERT IGNORE INTO seed_users (name, email, age, is_active, salary) VALUES (?, ?, ?, ?, ?)",
            )
            .bind(&names[i])
            .bind(&emails[i])
            .bind(ages[i])
            .bind(active_flags[i])
            .bind(salaries[i])
            .execute(&mut *tx)
            .await?;
        }
        tx.commit().await?;
    }
    Ok(count)
}

async fn seed_products(pool: &MySqlPool) -> Result<usize, sqlx::Error> {
    let count = 50;
    println!("  Seeding {} seed_products ...", count);
    let mut rng = rand::rng();

    for chunk in 0..(count + BATCH_SIZE - 1) / BATCH_SIZE {
        let batch_size = BATCH_SIZE.min(count - chunk * BATCH_SIZE);

        let mut names: Vec<String> = Vec::with_capacity(batch_size);
        let mut category_ids: Vec<i32> = Vec::with_capacity(batch_size);
        let mut prices: Vec<f64> = Vec::with_capacity(batch_size);
        let mut stocks: Vec<i32> = Vec::with_capacity(batch_size);
        let mut weights: Vec<Option<f64>> = Vec::with_capacity(batch_size);
        let mut descs: Vec<String> = Vec::with_capacity(batch_size);

        for _ in 0..batch_size {
            let product_types = [
                "Widget", "Gadget", "Tool", "Device", "Accessory", "Component", "Kit",
                "Module", "Instrument", "Appliance",
            ];
            let adj = ["Pro", "Max", "Mini", "Ultra", "Lite", "Plus", "Elite", "Basic"];
            names.push(format!(
                "{} {}-{}",
                product_types[rng.random_range(0..product_types.len())],
                rng.random_range(100..999),
                adj[rng.random_range(0..adj.len())]
            ));
            category_ids.push(rng.random_range(1..=CATEGORIES.len() as i32));
            let p: f64 = rng.random_range(5.0..500.0);
            prices.push((p * 100.0).round() / 100.0);
            stocks.push(rng.random_range(0..500));
            let w: f64 = rng.random_range(0.1..50.0);
            weights.push(Some((w * 100.0).round() / 100.0));
            descs.push(random_sentence(&mut rng, 5, 15));
        }

        let mut tx = pool.begin().await?;
        for i in 0..batch_size {
            sqlx::query(
                "INSERT INTO seed_products (name, category_id, price, in_stock, weight_kg, description) VALUES (?, ?, ?, ?, ?, ?)",
            )
            .bind(&names[i])
            .bind(category_ids[i])
            .bind(prices[i])
            .bind(stocks[i])
            .bind(weights[i])
            .bind(&descs[i])
            .execute(&mut *tx)
            .await?;
        }
        tx.commit().await?;
    }
    Ok(count)
}

async fn seed_tags(pool: &MySqlPool) -> Result<usize, sqlx::Error> {
    let tags = [
        ("new", "#00ff00"),
        ("sale", "#ff0000"),
        ("popular", "#0000ff"),
        ("limited", "#ff8800"),
        ("eco", "#00aa00"),
        ("premium", "#8800aa"),
        ("clearance", "#aa0000"),
        ("backorder", "#888888"),
        ("featured", "#ffaa00"),
        ("exclusive", "#aa00ff"),
        ("organic", "#00aa44"),
        ("handmade", "#aa44aa"),
        ("vintage", "#886600"),
        ("digital", "#0066aa"),
        ("local", "#44aa00"),
        ("imported", "#6600aa"),
        ("refurbished", "#666600"),
        ("certified", "#006644"),
        ("custom", "#aa6600"),
        ("seasonal", "#006688"),
    ];
    println!("  Seeding {} seed_tags ...", tags.len());

    for (name, color) in &tags {
        sqlx::query("INSERT IGNORE INTO seed_tags (name, color) VALUES (?, ?)")
            .bind(name)
            .bind(color)
            .execute(pool)
            .await?;
    }
    Ok(tags.len())
}

async fn seed_product_tags(pool: &MySqlPool) -> Result<usize, sqlx::Error> {
    let count = 150;
    println!("  Seeding {} seed_product_tags ...", count);
    let mut rng = rand::rng();

    let mut inserted = 0;
    let mut attempts = 0;
    while inserted < count && attempts < count * 3 {
        attempts += 1;
        let pid = rng.random_range(1..=50);
        let tid = rng.random_range(1..=20);
        match sqlx::query(
            "INSERT IGNORE INTO seed_product_tags (product_id, tag_id) VALUES (?, ?)",
        )
        .bind(pid)
        .bind(tid)
        .execute(pool)
        .await
        {
            Ok(r) => inserted += r.rows_affected() as usize,
            Err(_) => continue,
        }
    }
    Ok(inserted)
}

async fn seed_orders(pool: &MySqlPool) -> Result<usize, sqlx::Error> {
    let count = 200;
    println!("  Seeding {} seed_orders ...", count);
    let mut rng = rand::rng();

    let statuses = ["pending", "processing", "shipped", "delivered", "cancelled"];

    for chunk in 0..(count + BATCH_SIZE - 1) / BATCH_SIZE {
        let batch_size = BATCH_SIZE.min(count - chunk * BATCH_SIZE);
        let mut tx = pool.begin().await?;

        for _ in 0..batch_size {
            let user_id = rng.random_range(1..=100);
            let total_raw: f64 = rng.random_range(10.0..1000.0);
            let total = (total_raw * 100.0).round() / 100.0;
            let status = statuses[rng.random_range(0..statuses.len())];
            let addr = format!(
                "{} {} St, {}, {} {}",
                rng.random_range(100..9999),
                ["Main", "Oak", "Elm", "Maple", "Pine", "Cedar"][rng.random_range(0..6)],
                random_city(&mut rng),
                random_state(&mut rng),
                rng.random_range(10000..99999)
            );

            sqlx::query(
                "INSERT INTO seed_orders (user_id, total, status, shipping_address) VALUES (?, ?, ?, ?)",
            )
            .bind(user_id)
            .bind(total)
            .bind(status)
            .bind(&addr)
            .execute(&mut *tx)
            .await?;
        }
        tx.commit().await?;
    }
    Ok(count)
}

async fn seed_order_items(pool: &MySqlPool) -> Result<usize, sqlx::Error> {
    let count = 500;
    println!("  Seeding {} seed_order_items ...", count);
    let mut rng = rand::rng();

    for chunk in 0..(count + BATCH_SIZE - 1) / BATCH_SIZE {
        let batch_size = BATCH_SIZE.min(count - chunk * BATCH_SIZE);
        let mut tx = pool.begin().await?;

        for _ in 0..batch_size {
            let order_id = rng.random_range(1..=200);
            let product_id = rng.random_range(1..=50);
            let qty = rng.random_range(1..10);
            let price_raw: f64 = rng.random_range(5.0..500.0);
            let price = (price_raw * 100.0).round() / 100.0;

            sqlx::query(
                "INSERT INTO seed_order_items (order_id, product_id, quantity, unit_price) VALUES (?, ?, ?, ?)",
            )
            .bind(order_id)
            .bind(product_id)
            .bind(qty)
            .bind(price)
            .execute(&mut *tx)
            .await?;
        }
        tx.commit().await?;
    }
    Ok(count)
}

async fn seed_reviews(pool: &MySqlPool) -> Result<usize, sqlx::Error> {
    let count = 100;
    println!("  Seeding {} seed_reviews ...", count);
    let mut rng = rand::rng();

    let comments = [
        "Great product, highly recommended!",
        "Good value for the price.",
        "Decent quality, does the job.",
        "Not what I expected, but okay.",
        "Excellent! Will buy again.",
        "Average product, nothing special.",
        "Perfect, exactly as described.",
        "Fast shipping, good quality.",
        "Could be better for the price.",
        "Outstanding quality and service.",
    ];

    for chunk in 0..(count + BATCH_SIZE - 1) / BATCH_SIZE {
        let batch_size = BATCH_SIZE.min(count - chunk * BATCH_SIZE);
        let mut tx = pool.begin().await?;

        for _ in 0..batch_size {
            let product_id = rng.random_range(1..=50);
            let user_id = rng.random_range(1..=100);
            let rating = rng.random_range(1..=5);
            let comment = comments[rng.random_range(0..comments.len())];

            sqlx::query(
                "INSERT INTO seed_reviews (product_id, user_id, rating, comment) VALUES (?, ?, ?, ?)",
            )
            .bind(product_id)
            .bind(user_id)
            .bind(rating)
            .bind(comment)
            .execute(&mut *tx)
            .await?;
        }
        tx.commit().await?;
    }
    Ok(count)
}

async fn seed_events(pool: &MySqlPool) -> Result<usize, sqlx::Error> {
    let count = 200;
    println!("  Seeding {} seed_events ...", count);
    let mut rng = rand::rng();

    let types = [
        "page_view", "login", "logout", "purchase", "signup", "search", "click",
        "scroll", "form_submit", "error", "api_call", "download", "upload",
    ];

    for chunk in 0..(count + BATCH_SIZE - 1) / BATCH_SIZE {
        let batch_size = BATCH_SIZE.min(count - chunk * BATCH_SIZE);
        let mut tx = pool.begin().await?;

        for _ in 0..batch_size {
            let event_type = types[rng.random_range(0..types.len())];
            let payload = serde_json::json!({
                "ip": format!("{}.{}.{}.{}", rng.random_range(1..255), rng.random_range(0..255), rng.random_range(0..255), rng.random_range(1..255)),
                "user_agent": "Mozilla/5.0",
                "session_id": format!("sess_{}", rng.random_range(100000..999999)),
                "duration_ms": rng.random_range(50..5000),
                "referrer": "https://example.com",
            });

            sqlx::query("INSERT INTO seed_events (event_type, payload) VALUES (?, ?)")
                .bind(event_type)
                .bind(payload)
                .execute(&mut *tx)
                .await?;
        }
        tx.commit().await?;
    }
    Ok(count)
}

async fn seed_logs(pool: &MySqlPool) -> Result<usize, sqlx::Error> {
    let count = 1000;
    println!("  Seeding {} seed_logs ...", count);
    let mut rng = rand::rng();

    let levels = ["DEBUG", "INFO", "WARN", "ERROR"];
    let sources = [
        "app.server",
        "api.gateway",
        "db.migrator",
        "auth.service",
        "payment.worker",
        "search.indexer",
        "cache.updater",
        "email.sender",
        "file.processor",
        "sync.engine",
    ];

    for chunk in 0..(count + BATCH_SIZE - 1) / BATCH_SIZE {
        let batch_size = BATCH_SIZE.min(count - chunk * BATCH_SIZE);
        let mut tx = pool.begin().await?;

        for _ in 0..batch_size {
            let level = levels[rng.random_range(0..levels.len())];
            let source = sources[rng.random_range(0..sources.len())];
            let message = match level {
                "ERROR" => format!("Failed to process request: {}", random_sentence(&mut rng, 3, 8)),
                "WARN" => format!("Degraded performance detected: {}", random_sentence(&mut rng, 3, 8)),
                "INFO" => format!("Request completed: {}", random_sentence(&mut rng, 3, 8)),
                _ => format!("Debug: {}", random_sentence(&mut rng, 2, 6)),
            };

            sqlx::query("INSERT INTO seed_logs (level, message, source) VALUES (?, ?, ?)")
                .bind(level)
                .bind(&message)
                .bind(source)
                .execute(&mut *tx)
                .await?;
        }
        tx.commit().await?;
    }
    Ok(count)
}

async fn seed_audit_trail(pool: &MySqlPool) -> Result<usize, sqlx::Error> {
    let count = 500;
    println!("  Seeding {} seed_audit_trail ...", count);
    let mut rng = rand::rng();

    let tables = [
        "seed_users", "seed_products", "seed_orders", "seed_reviews",
    ];
    let operations = ["INSERT", "UPDATE", "DELETE"];
    let users = [
        "admin", "app_user", "migration_bot", "analytics_svc", "sync_worker",
    ];

    for chunk in 0..(count + BATCH_SIZE - 1) / BATCH_SIZE {
        let batch_size = BATCH_SIZE.min(count - chunk * BATCH_SIZE);
        let mut tx = pool.begin().await?;

        for _ in 0..batch_size {
            let table = tables[rng.random_range(0..tables.len())];
            let op = operations[rng.random_range(0..operations.len())];
            let record_id = rng.random_range(1..500);
            let old_data: Option<serde_json::Value> = if op != "INSERT" {
                Some(serde_json::json!({"status": "previous", "timestamp": "2024-01-01T00:00:00Z"}))
            } else {
                None
            };
            let new_data: Option<serde_json::Value> = if op != "DELETE" {
                Some(serde_json::json!({"status": "current", "timestamp": "2024-06-01T00:00:00Z"}))
            } else {
                None
            };
            let changed_by = users[rng.random_range(0..users.len())];

            sqlx::query(
                "INSERT INTO seed_audit_trail (table_name, operation, record_id, old_data, new_data, changed_by) VALUES (?, ?, ?, ?, ?, ?)",
            )
            .bind(table)
            .bind(op)
            .bind(record_id)
            .bind(&old_data)
            .bind(&new_data)
            .bind(changed_by)
            .execute(&mut *tx)
            .await?;
        }
        tx.commit().await?;
    }
    Ok(count)
}

async fn seed_addresses(pool: &MySqlPool) -> Result<usize, sqlx::Error> {
    let count = 100;
    println!("  Seeding {} seed_addresses ...", count);
    let mut rng = rand::rng();

    for chunk in 0..(count + BATCH_SIZE - 1) / BATCH_SIZE {
        let batch_size = BATCH_SIZE.min(count - chunk * BATCH_SIZE);
        let mut tx = pool.begin().await?;

        for _ in 0..batch_size {
            let user_id = rng.random_range(1..=100);
            let street_num = rng.random_range(100..9999);
            let street_names = [
                "Main St", "Oak Ave", "Elm St", "Maple Dr", "Pine Ln", "Cedar Ct",
                "Birch Rd", "Walnut Way", "Cherry Blvd", "Willow Ln",
            ];
            let street = format!("{} {}", street_num, street_names[rng.random_range(0..street_names.len())]);
            let city = random_city(&mut rng);
            let state = random_state(&mut rng);
            let zip = random_zip(&mut rng);
            let is_primary = if rng.random_bool(0.3) { 1 } else { 0 };

            sqlx::query(
                "INSERT INTO seed_addresses (user_id, street, city, state, zip, is_primary) VALUES (?, ?, ?, ?, ?, ?)",
            )
            .bind(user_id)
            .bind(&street)
            .bind(&city)
            .bind(&state)
            .bind(&zip)
            .bind(is_primary)
            .execute(&mut *tx)
            .await?;
        }
        tx.commit().await?;
    }
    Ok(count)
}
