#![allow(dead_code)]
use anyhow::Result;
use http::{Request, Response};
use spin_sdk::{
    http_component,
    pg::{self, Decode},
};
use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream};

#[macro_use]
extern crate serde_derive;

const DB_URL_ENV: &str = "DB_URL";

#[derive(Serialize, Deserialize)]
struct User {
    id: Option<i32>,
    name: String,
    email: String,
}

const OK_RESPONSE: &str = "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\n\r\n";
const NOT_FOUND: &str = "HTTP/1.1 404 NOT FOUND\r\n\r\n";
const INTERNAL_SERVER_ERROR: &str = "HTTP/1.1 500 INTERNAL SERVER ERROR\r\n\r\n";

fn main() {
    if let Err(e) = set_database() {
        eprintln!("Error setting up database: {}", e);
        return;
    }

    let listener = TcpListener::bind(format!("0.0.0.0:8080")).unwrap();
    println!("Server started at port 8080");

    //handle the client
    for stream in listener.incoming() {
        match stream {
            Ok(stream) => {
                handle_client(stream);
            }
            Err(e) => {
                println!("Error: {}", e);
            }
        }
    }
}

fn handle_client(mut stream: TcpStream) {
    let mut buffer = [0; 1024];
    let mut request = String::new();

    match stream.read(&mut buffer) {
        Ok(size) => {
            request.push_str(String::from_utf8_lossy(&buffer[..size]).as_ref());

            let (status_line, content) = match &*request {
                r if r.starts_with("POST /users") => handle_post_request(r),
                r if r.starts_with("PUT /users/") => handle_put_request(r),
                r if r.starts_with("DELETE /users/") => handle_delete_request(r),
                _ => (NOT_FOUND.to_string(), "404 Not Found".to_string()),
            };

            stream
                .write_all(format!("{}{}", status_line, content).as_bytes())
                .unwrap();
        }
        Err(e) => {
            println!("Error: {}", e);
        }
    }
}

fn handle_post_request(request: &str) -> (String, String) {
    match (
        get_user_request_body(&request),
        pg::Connection::open(DB_URL_ENV),
    ) {
        (Ok(user), Ok(mut client)) => {
            // Convert &String to String
            let name = user.name.clone();
            let email = user.email.clone();

            client
                .execute(
                    "INSERT INTO users (name, email) VALUES ($1, $2)",
                    &[spin_sdk::pg::ParameterValue::Str(name.clone()), spin_sdk::pg::ParameterValue::Str(email.clone())],  
                )
                .unwrap();

            (OK_RESPONSE.to_string(), "User created".to_string())
        }
        _ => (INTERNAL_SERVER_ERROR.to_string(), "Error".to_string()),
    }
}





fn handle_put_request(request: &str) -> (String, String) {
    match (
        get_id(&request).parse::<i32>(),
        get_user_request_body(&request),
        pg::Connection::open(DB_URL_ENV),
    ) {
        (Ok(id), Ok(user), Ok(mut client)) => {
            let name = user.name.clone();
            let email = user.email.clone();

            client
                .execute(
                    "UPDATE users SET name = $1, email = $2 WHERE id = $3",
                    &[
                        spin_sdk::pg::ParameterValue::Str(name.clone()),
                        spin_sdk::pg::ParameterValue::Str(email.clone()),
                        spin_sdk::pg::ParameterValue::Int32(id),
                    ],
                )
                .unwrap();

            (OK_RESPONSE.to_string(), "User updated".to_string())
        }
        _ => (INTERNAL_SERVER_ERROR.to_string(), "Error".to_string()),
    }
}


fn handle_delete_request(request: &str) -> (String, String) {
    match (
        get_id(&request).parse::<i32>(),
        pg::Connection::open(DB_URL_ENV),
    ) {
        (Ok(id), Ok(mut client)) => {
            let rows_affected = client
                .execute("DELETE FROM users WHERE id = $1", &[spin_sdk::pg::ParameterValue::Int32(id)])
                .unwrap();

            if rows_affected == 0 {
                return (NOT_FOUND.to_string(), "User not found".to_string());
            }

            (OK_RESPONSE.to_string(), "User deleted".to_string())
        }
        _ => (INTERNAL_SERVER_ERROR.to_string(), "Error".to_string()),
    }
}

fn set_database() -> Result<()> {
    // Connect to the database
    let mut client = pg::Connection::open(DB_URL_ENV)?;

    // SQL query to create the users table if it doesn't exist
    let sql = "CREATE TABLE IF NOT EXISTS users (
        id SERIAL PRIMARY KEY,
        name VARCHAR NOT NULL,
        email VARCHAR NOT NULL
    )";

    // Execute the SQL query to create the table
    client.execute(sql, &[])?;

    // Database setup successful
    Ok(())
}

fn get_id(request: &str) -> &str {
    request
        .split("/")
        .nth(2)
        .unwrap_or_default()
        .split_whitespace()
        .next()
        .unwrap_or_default()
}

fn get_user_request_body(request: &str) -> Result<User, serde_json::Error> {
    serde_json::from_str(request.split("\r\n\r\n").last().unwrap_or_default())
}
