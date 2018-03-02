extern crate actix;
extern crate actix_web;
#[macro_use]
extern crate slog;
extern crate slog_async;
extern crate slog_term;
extern crate uuid;

use slog::Drain;

mod common {
    use slog::Logger;

    pub trait State {
        fn log(&self) -> &Logger;
    }

    pub struct StateImpl {
        pub log: Logger,
    }

    impl State for StateImpl {
        fn log(&self) -> &Logger {
            &self.log
        }
    }
}

mod middleware {
    use actix_web;
    use actix_web::{HttpRequest, HttpResponse};
    use actix_web::middleware::Middleware as ActixMiddleware;
    use actix_web::middleware::{Response, Started};
    use common;
    use slog::Logger;

    pub mod log_initializer {
        use middleware::*;

        pub struct Middleware;

        /// The extension registered by this middleware to the request to make
        /// a `Logger `accessible.
        pub struct Extension(pub Logger);

        impl<S: common::State> ActixMiddleware<S> for Middleware {
            fn start(
                &self,
                req: &mut HttpRequest<S>,
            ) -> actix_web::Result<Started> {
                let log = req.state().log().clone();
                req.extensions().insert(Extension(log));
                Ok(Started::Done)
            }

            fn response(
                &self,
                _req: &mut HttpRequest<S>,
                resp: HttpResponse,
            ) -> actix_web::Result<Response> {
                Ok(Response::Done(resp))
            }
        }
    }

    pub mod request_id {
        use middleware::*;
        use uuid::Uuid;

        pub struct Middleware;

        /// The extension registered by this middleware to the request to make
        /// a request ID accessible.
        pub struct Extension(pub String);

        impl<S: common::State> ActixMiddleware<S> for Middleware {
            fn start(
                &self,
                req: &mut HttpRequest<S>,
            ) -> actix_web::Result<Started> {
                let request_id = Uuid::new_v4().simple().to_string();
                req.extensions().insert(Extension(request_id.clone()));

                // Remove the request's original `Logger`
                let log = req.extensions()
                    .remove::<log_initializer::Extension>()
                    .unwrap()
                    .0;

                debug!(&log, "Generated request ID";
                    "request_id" => request_id.as_str());

                // Insert a new `Logger` that includes the generated request ID
                req.extensions().insert(log_initializer::Extension(log.new(
                    o!("request_id" => request_id),
                )));

                Ok(Started::Done)
            }

            fn response(
                &self,
                _req: &mut HttpRequest<S>,
                resp: HttpResponse,
            ) -> actix_web::Result<Response> {
                Ok(Response::Done(resp))
            }
        }
    }
}

fn log() -> slog::Logger {
    let decorator = slog_term::TermDecorator::new().build();
    let drain = slog_term::FullFormat::new(decorator).build().fuse();
    let drain = slog_async::Async::new(drain).build().fuse();
    slog::Logger::root(drain, o!("app" => "middleware-rust"))
}

fn main() {
    let sys = actix::System::new("middleware-rust");

    let _addr = actix_web::HttpServer::new(|| {
        actix_web::Application::with_state(common::StateImpl { log: log() })
            .middleware(middleware::log_initializer::Middleware)
            .middleware(middleware::request_id::Middleware)
            .resource("/", |r| {
                r.method(actix_web::Method::GET)
                    .f(|_req| actix_web::httpcodes::HTTPOk)
            })
    }).bind("127.0.0.1:8080")
        .expect("Can not bind to 127.0.0.1:8080")
        .start();

    println!("Starting http server: 127.0.0.1:8080");
    let _ = sys.run();
}
