use std::time::Instant;

use redis::aio::MultiplexedConnection;
use redis::RedisResult;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<String> = std::env::args().collect();
    let uri = if args.len() > 1 {
        &args[1]
    } else {
        "redis://127.0.0.1:6379"
    };

    println!("Connecting to: {}", uri);
    let client = redis::Client::open(uri)?;
    let conn = client.get_multiplexed_async_connection().await?;
    let start = Instant::now();

    seed_db0_strings(&conn).await?;
    seed_db1_hashes(&conn).await?;
    seed_db2_lists(&conn).await?;
    seed_db3_sets(&conn).await?;
    seed_db4_sorted_sets(&conn).await?;
    seed_db5_streams(&conn).await?;

    println!(
        "\nDone — seeded 6 Redis databases in {:?}",
        start.elapsed()
    );
    Ok(())
}

async fn select(conn: &mut MultiplexedConnection, db: u8) -> RedisResult<()> {
    redis::cmd("SELECT").arg(db).query_async(conn).await
}

async fn seed_db0_strings(conn: &MultiplexedConnection) -> Result<(), Box<dyn std::error::Error>> {
    println!("  Seeding db0 (strings) ...");
    let mut conn = conn.clone();
    select(&mut conn, 0).await?;

    let pairs = [
        ("greeting", "Hello from dbrusty!"),
        ("app:name", "dbrusty"),
        ("app:version", "0.1.0"),
        ("app:description", "Terminal Database Client"),
        ("config:theme", "dark"),
        ("config:language", "en"),
        ("config:timeout", "30"),
        ("stats:uptime", "3600"),
        ("stats:queries", "42"),
        ("stats:connections", "7"),
        ("welcome:title", "dbrusty — TUI Database Client"),
        ("welcome:subtitle", "Explore. Query. Manage."),
        ("feature:redis", "enabled"),
        ("feature:mongodb", "enabled"),
        ("feature:postgres", "enabled"),
        ("feature:mysql", "enabled"),
        ("feature:mariadb", "enabled"),
        ("feature:sqlite", "enabled"),
        ("env:mode", "development"),
        ("env:host", "localhost"),
    ];
    for (key, val) in &pairs {
        redis::cmd("SET")
            .arg(key)
            .arg(val)
            .query_async::<()>(&mut conn)
            .await?;
    }
    println!("    {} keys", pairs.len());
    Ok(())
}

async fn seed_db1_hashes(conn: &MultiplexedConnection) -> Result<(), Box<dyn std::error::Error>> {
    println!("  Seeding db1 (hashes) ...");
    let mut conn = conn.clone();
    select(&mut conn, 1).await?;

    let users = [
        (1, "Alice", "alice@example.com", "admin", "active"),
        (2, "Bob", "bob@example.com", "user", "active"),
        (3, "Charlie", "charlie@example.com", "user", "inactive"),
        (4, "Diana", "diana@example.com", "moderator", "active"),
        (5, "Eve", "eve@example.com", "user", "active"),
    ];
    for (id, name, email, role, status) in &users {
        redis::cmd("HSET")
            .arg(format!("user:{}", id))
            .arg("name")
            .arg(name)
            .arg("email")
            .arg(email)
            .arg("role")
            .arg(role)
            .arg("status")
            .arg(status)
            .query_async::<()>(&mut conn)
            .await?;
    }

    let sessions = [
        ("sess:abc123", "user:1", "2024-06-01T10:00:00Z", "192.168.1.1"),
        ("sess:def456", "user:2", "2024-06-01T10:05:00Z", "192.168.1.2"),
        ("sess:ghi789", "user:3", "2024-06-01T10:10:00Z", "192.168.1.3"),
    ];
    for (sid, uid, login, ip) in &sessions {
        redis::cmd("HSET")
            .arg(sid)
            .arg("user_id")
            .arg(uid)
            .arg("login_at")
            .arg(login)
            .arg("ip")
            .arg(ip)
            .query_async::<()>(&mut conn)
            .await?;
    }
    println!("    {} hashes", users.len() + sessions.len());
    Ok(())
}

async fn seed_db2_lists(conn: &MultiplexedConnection) -> Result<(), Box<dyn std::error::Error>> {
    println!("  Seeding db2 (lists) ...");
    let mut conn = conn.clone();
    select(&mut conn, 2).await?;

    let jobs = ["process_payments", "send_emails", "generate_reports", "sync_data", "backup_db", "cleanup_logs", "update_indexes", "run_analytics"];
    for job in &jobs {
        redis::cmd("RPUSH")
            .arg("queue:jobs")
            .arg(job)
            .query_async::<()>(&mut conn)
            .await?;
    }

    let logs = [
        "Server started on port 8080",
        "Connected to database postgres://localhost:5432",
        "User alice logged in from 192.168.1.1",
        "Query executed: SELECT * FROM users LIMIT 10",
        "Cache miss for key 'user:42'",
        "Rate limit exceeded for IP 10.0.0.5",
        "Webhook sent to https://hooks.example.com/events",
        "Scheduled job 'cleanup_logs' completed in 1.2s",
        "Replication lag detected: 250ms",
        "Memory usage at 72% — within normal range",
    ];
    for log in &logs {
        redis::cmd("RPUSH")
            .arg("log:recent")
            .arg(log)
            .query_async::<()>(&mut conn)
            .await?;
    }
    println!("    {} items across 2 lists", jobs.len() + logs.len());
    Ok(())
}

async fn seed_db3_sets(conn: &MultiplexedConnection) -> Result<(), Box<dyn std::error::Error>> {
    println!("  Seeding db3 (sets) ...");
    let mut conn = conn.clone();
    select(&mut conn, 3).await?;

    let tags = ["rust", "redis", "tui", "database", "cli", "terminal", "async", "tokio", "ratatui", "crossterm", "sql", "nosql", "postgres", "mysql", "mongodb", "sqlite", "mariadb", "performance", "tools", "devops"];
    for tag in &tags {
        redis::cmd("SADD")
            .arg("tags:dbrusty")
            .arg(tag)
            .query_async::<()>(&mut conn)
            .await?;
    }

    let online = ["user:1", "user:2", "user:4", "user:5"];
    for u in &online {
        redis::cmd("SADD")
            .arg("users:online")
            .arg(u)
            .query_async::<()>(&mut conn)
            .await?;
    }
    println!("    {} items across 2 sets", tags.len() + online.len());
    Ok(())
}

async fn seed_db4_sorted_sets(conn: &MultiplexedConnection) -> Result<(), Box<dyn std::error::Error>> {
    println!("  Seeding db4 (sorted sets) ...");
    let mut conn = conn.clone();
    select(&mut conn, 4).await?;

    let leaderboard = [
        (100.0, "alice"),
        (85.0, "bob"),
        (72.0, "charlie"),
        (61.0, "diana"),
        (55.0, "eve"),
        (48.0, "frank"),
        (42.0, "grace"),
        (37.0, "henry"),
        (29.0, "iris"),
        (18.0, "jack"),
    ];
    for (score, member) in &leaderboard {
        redis::cmd("ZADD")
            .arg("leaderboard:global")
            .arg(*score)
            .arg(member)
            .query_async::<()>(&mut conn)
            .await?;
    }

    let pageviews = [
        (1200.0, "/"),
        (850.0, "/docs"),
        (620.0, "/api"),
        (410.0, "/about"),
        (380.0, "/settings"),
        (290.0, "/help"),
        (210.0, "/changelog"),
        (150.0, "/license"),
    ];
    for (score, page) in &pageviews {
        redis::cmd("ZADD")
            .arg("analytics:pageviews")
            .arg(*score)
            .arg(page)
            .query_async::<()>(&mut conn)
            .await?;
    }
    println!(
        "    {} items across 2 sorted sets",
        leaderboard.len() + pageviews.len()
    );
    Ok(())
}

async fn seed_db5_streams(conn: &MultiplexedConnection) -> Result<(), Box<dyn std::error::Error>> {
    println!("  Seeding db5 (streams) ...");
    let mut conn = conn.clone();
    select(&mut conn, 5).await?;

    let events = [
        ("event:login", &[("user", "alice"), ("ip", "192.168.1.1"), ("device", "desktop")] as &[(&str, &str)]),
        ("event:purchase", &[("user", "bob"), ("item", "widget-123"), ("amount", "29.99")]),
        ("event:logout", &[("user", "alice"), ("session_ms", "3600000")]),
        ("event:signup", &[("user", "charlie"), ("referrer", "docs.example.com")]),
        ("event:error", &[("service", "api"), ("code", "500"), ("path", "/users")]),
    ];
    for (stream, fields) in &events {
        let mut cmd = redis::Cmd::new();
        cmd.arg("XADD").arg(stream).arg("*");
        for (k, v) in *fields {
            cmd.arg(k);
            cmd.arg(v);
        }
        cmd.query_async::<()>(&mut conn).await?;
    }
    println!("    {} streams", events.len());
    Ok(())
}
