#[macro_use]
extern crate serde_derive;

use axum::{
    http::StatusCode,
    response::IntoResponse,
    routing::{get, post},
    Json, Router,
};
use http::Method;
use iqengine_plugin::models::{
    CustomParamType, FunctionParams, FunctionParamsBuilder, FunctionResult, Annotation, CustomParamsList,
};
use serde::{Deserialize, Serialize};
use std::{collections::HashMap, net::SocketAddr};
use tower::ServiceBuilder;
use tower_http::cors::{Any, CorsLayer};

#[tokio::main]
async fn main() {
    // initialize tracing
    tracing_subscriber::fmt::init();

    let cors = CorsLayer::new()
        .allow_origin(Any)
        .allow_headers(Any)
        // .allow_credentials(true)
        .allow_methods(vec![Method::GET, Method::POST])
        ;

    // build our application with a route
    let app = Router::new()
        .route("/plugins", get(get_functions_list))
        .route("/plugins/", get(get_functions_list))
        .route("/plugins/fm-receiver", get(get_fm_receiver_params))
        .route("/plugins/fm-receiver", post(post_fm_receiver))
        .layer(ServiceBuilder::new().layer(cors));

    let addr = SocketAddr::from(([127, 0, 0, 1], 8000));
    println!("listening on {}", addr);
    axum::Server::bind(&addr)
        .serve(app.into_make_service())
        .await
        .unwrap();
}

// Return list of IQEngine functions
async fn get_functions_list() -> (StatusCode, Json<Vec<&'static str>>) {
    let functions_list = vec!["fm-receiver"];
    (StatusCode::OK, Json(functions_list))
}

// Describe the parameters for the fm-receiver
async fn get_fm_receiver_params() -> (StatusCode, Json<CustomParamsList>) {
    let func_params = FunctionParamsBuilder::new()
        .max_inputs(1)
        .max_outputs(1)
        .custom_param(
            "center_freq",
            "Center of FM carrier",
            CustomParamType::Number,
            Some("0"),
        )
        .build();
    (StatusCode::OK, Json(func_params.custom_params))
}

// Apply the fm-receiver
async fn post_fm_receiver() -> (StatusCode, Json<FunctionResult>) {
    let mut result = FunctionResult::new();
    let mut first_annot = Annotation::new(100, 10);
    first_annot.core_colon_label = Some("random detection".into());
    first_annot.core_colon_comment = Some("from rust plugin".into());
    let mut annotations = Vec::new();
    annotations.push(first_annot);
    result.annotations = Some(annotations);
    (StatusCode::OK, Json(result))
}
