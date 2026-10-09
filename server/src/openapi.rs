use crate::modules::{
    auth::dto::{Credentials, PublicUser},
    catalog::dto::ProductInput,
    orders::dto::CreateOrderInput,
    payments::dto::{ApproveInput, RejectInput},
    tutorials::dto::{ChapterInput, TutorialInput},
};
use serde_json::{Value, json};
use utoipa::OpenApi;
#[derive(OpenApi)]
#[openapi(
    info(
        title = "DevStore API",
        version = "1.0.0",
        description = "Scope A: tutorial checkout. Cookie authentication; mutations require exact Origin and X-CSRF-Token. All responses are private/no-store. Amounts are integer CNY cents."
    ),
    components(schemas(
        Credentials,
        PublicUser,
        ProductInput,
        TutorialInput,
        ChapterInput,
        CreateOrderInput,
        ApproveInput,
        RejectInput
    ))
)]
struct ApiDoc;
/// Routing metadata is kept here; input schemas come from the same DTOs as handlers.
pub async fn document() -> axum::Json<Value> {
    let mut doc = serde_json::to_value(ApiDoc::openapi()).expect("OpenAPI is serializable");
    doc["servers"] = json!([{"url":"/api/v1"}]);
    doc["components"]["securitySchemes"] = json!({"session":{"type":"apiKey","in":"cookie","name":"ds_access"},"refresh":{"type":"apiKey","in":"cookie","name":"ds_refresh"}});
    doc["components"]["schemas"]["Envelope"] = json!({"type":"object","required":["code","message","data","request_id"],"properties":{"code":{"type":"string"},"message":{"type":"string"},"data":{},"request_id":{"type":"string","format":"uuid"}}});
    doc["components"]["schemas"]["Receipt"] = json!({"type":"object","required":["file"],"properties":{"file":{"type":"string","format":"binary","description":"One PNG/JPEG, max 5 MiB, each dimension <=8000, <=20 million pixels. Full multipart request <=6 MiB. ClamAV must return clean."}}});
    let routes = [
        ("get", "/health/live", "Liveness", "", false),
        ("get", "/health/ready", "Dependency readiness", "", false),
        ("post", "/auth/register", "Register", "Credentials", false),
        ("post", "/auth/login", "Login", "Credentials", false),
        ("post", "/auth/refresh", "Rotate session", "", true),
        ("post", "/auth/logout", "Revoke session", "", true),
        ("get", "/auth/csrf", "Get CSRF token", "", true),
        ("get", "/users/me", "Current user", "", true),
        ("get", "/categories", "Categories", "", false),
        ("get", "/products", "Published products", "", false),
        ("get", "/products/{slug}", "Product", "", false),
        ("get", "/tutorials", "Published tutorials", "", false),
        (
            "get",
            "/tutorials/{slug}",
            "Tutorial metadata without body",
            "",
            false,
        ),
        (
            "get",
            "/tutorials/{id}/chapters/{chapter_id}",
            "Preview or entitled chapter",
            "",
            false,
        ),
        (
            "get",
            "/me/library",
            "Active tutorial entitlements",
            "",
            true,
        ),
        (
            "post",
            "/orders",
            "Idempotent checkout",
            "CreateOrderInput",
            true,
        ),
        ("get", "/orders", "My orders", "", true),
        ("get", "/orders/{id}", "My order", "", true),
        (
            "post",
            "/orders/{id}/cancel",
            "Cancel pending order",
            "",
            true,
        ),
        (
            "get",
            "/orders/{id}/payment-instructions",
            "Snapshot payment instructions",
            "",
            true,
        ),
        (
            "post",
            "/orders/{id}/payment-proof",
            "Submit scanned receipt",
            "Receipt",
            true,
        ),
        (
            "get",
            "/orders/{id}/payment-proof/{proof}",
            "Private receipt",
            "",
            true,
        ),
        ("get", "/admin/products", "All products", "", true),
        (
            "post",
            "/admin/products",
            "Create product",
            "ProductInput",
            true,
        ),
        ("get", "/admin/products/{id}", "Product editor", "", true),
        (
            "patch",
            "/admin/products/{id}",
            "Update product",
            "ProductInput",
            true,
        ),
        ("post", "/admin/categories", "Create category", "", true),
        ("get", "/admin/tutorials", "All tutorials", "", true),
        (
            "post",
            "/admin/tutorials",
            "Create tutorial",
            "TutorialInput",
            true,
        ),
        (
            "get",
            "/admin/tutorials/{id}",
            "Tutorial editor including bodies",
            "",
            true,
        ),
        (
            "patch",
            "/admin/tutorials/{id}",
            "Update tutorial",
            "TutorialInput",
            true,
        ),
        (
            "post",
            "/admin/tutorials/{id}/chapters",
            "Create chapter",
            "ChapterInput",
            true,
        ),
        (
            "patch",
            "/admin/tutorials/{id}/chapters/{chapter_id}",
            "Update chapter",
            "ChapterInput",
            true,
        ),
        ("get", "/admin/orders", "All orders", "", true),
        ("get", "/admin/orders/{id}", "Order detail", "", true),
        ("get", "/admin/payments", "Payment review queue", "", true),
        (
            "get",
            "/admin/payments/{proof}/proof",
            "Private receipt for review",
            "",
            true,
        ),
        (
            "post",
            "/admin/payments/{proof}/approve",
            "Confirm actual receipt and grant access",
            "ApproveInput",
            true,
        ),
        (
            "post",
            "/admin/payments/{proof}/reject",
            "Reject with reason",
            "RejectInput",
            true,
        ),
        ("get", "/admin/audit-logs", "Admin audit", "", true),
    ];
    for (method, path, title, schema, auth) in routes {
        let mut op = json!({"summary":title,"responses":{"200":{"description":"Success","content":{"application/json":{"schema":{"$ref":"#/components/schemas/Envelope"}}}},"400":{"description":"Malformed request"},"401":{"description":"Unauthenticated or expired session"},"403":{"description":"CSRF, role, or entitlement denied"},"404":{"description":"Missing or owned by another user"},"409":{"description":"State/idempotency/reference conflict"},"413":{"description":"Upload too large"},"422":{"description":"Validation failed"},"429":{"description":"Rate limited"},"500":{"description":"Transaction failed"},"503":{"description":"Dependency/payment configuration unavailable"}},"parameters":[]});
        if method == "post"
            && !path.ends_with("/approve")
            && !path.ends_with("/reject")
            && !path.ends_with("/cancel")
            && !path.starts_with("/auth/login")
            && !path.starts_with("/auth/logout")
            && !path.starts_with("/auth/refresh")
        {
            op["responses"]["201"] = json!({"description":"Created","content":{"application/json":{"schema":{"$ref":"#/components/schemas/Envelope"}}}});
        }
        let mut params = vec![];
        for segment in path.split('/').filter(|s| s.starts_with('{')) {
            let name = segment.trim_matches(['{', '}']);
            params
                .push(json!({"name":name,"in":"path","required":true,"schema":{"type":"string"}}));
        }
        if auth {
            op["security"] = if path == "/auth/refresh" {
                json!([{"refresh":[]}])
            } else {
                json!([{"session":[]}])
            };
        }
        if method != "get" {
            params.push(json!({"name":"Origin","in":"header","required":true,"schema":{"type":"string"},"description":"Must equal configured SITE_URL"}));
            if auth {
                params.push(json!({"name":"X-CSRF-Token","in":"header","required":true,"schema":{"type":"string"}}));
            }
        }
        if method == "post" && path == "/orders" {
            params.push(json!({"name":"Idempotency-Key","in":"header","required":true,"schema":{"type":"string","minLength":16,"maxLength":128},"description":"Visible ASCII, scoped to user. Keep stable across retries."}));
        }
        if !schema.is_empty() {
            let mime = if schema == "Receipt" {
                "multipart/form-data"
            } else {
                "application/json"
            };
            op["requestBody"] = json!({"required":true,"content":{mime:{"schema":{"$ref":format!("#/components/schemas/{schema}")}}}});
            if method == "patch" {
                let partial = format!("{schema}Patch");
                let mut s = doc["components"]["schemas"][schema].clone();
                s.as_object_mut().expect("object schema").remove("required");
                doc["components"]["schemas"][&partial] = s;
                op["requestBody"]["content"][mime]["schema"]["$ref"] =
                    json!(format!("#/components/schemas/{partial}"));
            }
        }
        if path.ends_with("/proof") && method == "get" || path.contains("/payment-proof/") {
            op["responses"]["200"]["content"] = json!({"image/png":{"schema":{"type":"string","format":"binary"}},"image/jpeg":{"schema":{"type":"string","format":"binary"}}});
        }
        op["parameters"] = json!(params);
        doc["paths"][path][method] = op;
    }
    axum::Json(doc)
}
