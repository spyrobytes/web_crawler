# Actix Web Framework

[Actix](https://actix.rs/)

Actix Web is a powerful, pragmatic, and extremely fast web framework for Rust.
It lets you quickly and confidently develop *web services* in Rust. The framework
is available in the `actix-web`. `actix-web` provides various primitives to build 
web servers and applications with Rust. It provides routing, middleware,
pre-processing of requests, post-processing of responses, etc.

## How it works

An application developed with Actix Web will expose an HTTP server contained 
within a native executable. You can either put this behind another HTTP server 
like `nginx` or serve it up as-is. Even in the complete absence of another HTTP 
server Actix Web is powerful enough to provide HTTP/1 and HTTP/2 support as well 
as TLS (HTTPS). This makes it useful for building small services ready for 
production. 

All `actix-web` servers are built around the `App` instance. It is used for 
registering routes for resources and middleware. It also stores application 
state shared across all handlers within the same scope.

## Basic *getting started* example

```rust
use actix_web::{get, post, web, App, HttpResponse, HttpServer, Responder};

// Request handlers use `async` functions that accept zero or more 
// parameters.
// 1. Using built-in macros: specify a path and handler
#[get("/")]
async fn hello() -> impl Responder {
    HttpResponse::Ok().body("Hello world!")
}

#[post("/echo")]
async fn echo(req_body: String) -> impl Responder {
    HttpResponse::Ok().body(req_body)
}

// 2. Without macros: use `App::service()` to register the request handler
async fn manual_hello() -> impl Responder {
    HttpResponse::Ok().body("Hey there!")
}

#[actix_web::main]
async fn main() -> std::io::Result<()> {
    // create an App instance and register the request handlers.
    async fn main() -> std::io::Result<()> {
    HttpServer::new(|| {
        App::new()
            .service(hello)
            .service(echo)
            // use `App::route()` to register an HTTP method and path
            .route("/hey", web::get().to(manual_hello))
    })
        .bind(("127.0.0.1", 8080))?
        .run()
        .await 
    }
}

```
