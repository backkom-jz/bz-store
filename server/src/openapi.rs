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
        description = "Tutorial, digital product and service checkout; manual payment review and refund registration; private downloads, tickets and blog publishing. Cookie authentication; mutations require exact Origin and X-CSRF-Token. All responses are private/no-store. Amounts are integer CNY cents."
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
    doc["components"]["schemas"]["Receipt"] = json!({"type":"object","required":["file"],"properties":{"file":{"type":"string","format":"binary","description":"One fully decodable PNG/JPEG, max 5 MiB, each dimension <=8000, <=20 million pixels. Full multipart request <=6 MiB. Stored privately; not malware-scanned."}}});
    doc["components"]["schemas"]["DigitalAsset"] = json!({"type":"object","required":["file"],"properties":{"file":{"type":"string","format":"binary","description":"Private PDF or ZIP, maximum 5 MiB. Files cannot be replaced after the product has orders."}}});
    doc["components"]["schemas"]["RefundInput"] = json!({"type":"object","additionalProperties":false,"required":["amount_cents","reference_no","note"],"properties":{"amount_cents":{"type":"integer","minimum":0,"maximum":100000000},"reference_no":{"type":"string","maxLength":128},"note":{"type":"string","maxLength":2000}}});
    doc["components"]["schemas"]["TicketMessage"] = json!({"type":"object","additionalProperties":false,"required":["body"],"properties":{"body":{"type":"string","maxLength":4000}}});
    doc["components"]["schemas"]["BlogInput"] = json!({"type":"object","additionalProperties":false,"required":["title","slug","excerpt","content_md","status"],"properties":{"title":{"type":"string","maxLength":200},"slug":{"type":"string","pattern":"^[a-z0-9]+(-[a-z0-9]+)*$","maxLength":220},"excerpt":{"type":"string","maxLength":500},"content_md":{"type":"string","maxLength":100000},"status":{"type":"string","enum":["draft","published"]}}});
    let routes = [
        (
            "post",
            "/admin/products/{id}/file",
            "Attach private digital file",
            "DigitalAsset",
            true,
        ),
        (
            "get",
            "/me/downloads/{id}",
            "Download file after checking current ownership",
            "",
            true,
        ),
        (
            "post",
            "/admin/orders/{id}/refund",
            "Record an externally completed whole-order refund",
            "RefundInput",
            true,
        ),
        ("get", "/me/tickets", "My service tickets", "", true),
        (
            "get",
            "/me/tickets/{id}",
            "My ticket and messages",
            "",
            true,
        ),
        (
            "post",
            "/me/tickets/{id}/messages",
            "Send ticket message",
            "TicketMessage",
            true,
        ),
        (
            "post",
            "/me/tickets/{id}/accept",
            "Accept service delivery",
            "",
            true,
        ),
        ("get", "/admin/tickets", "All service tickets", "", true),
        (
            "get",
            "/admin/tickets/{id}",
            "Ticket and messages for administration",
            "",
            true,
        ),
        (
            "post",
            "/admin/tickets/{id}/messages",
            "Reply to service ticket",
            "TicketMessage",
            true,
        ),
        ("get", "/blog", "Published article summaries", "", false),
        ("get", "/blog/{slug}", "Published article", "", false),
        ("get", "/admin/blog", "All articles", "", true),
        ("post", "/admin/blog", "Create article", "BlogInput", true),
        ("get", "/admin/blog/{id}", "Article editor", "", true),
        (
            "patch",
            "/admin/blog/{id}",
            "Replace article fields",
            "BlogInput",
            true,
        ),
        ("get", "/health/live", "Liveness", "", false),
        ("get", "/health/ready", "Dependency readiness", "", false),
        ("post", "/auth/register", "Register", "Credentials", false),
        (
            "post",
            "/auth/verify-email",
            "Verify email",
            "TokenInput",
            false,
        ),
        (
            "post",
            "/auth/verify-email/resend",
            "Resend verification",
            "EmailInput",
            false,
        ),
        (
            "post",
            "/auth/password/forgot",
            "Request password reset",
            "EmailInput",
            false,
        ),
        (
            "post",
            "/auth/password/reset",
            "Reset password",
            "ResetInput",
            false,
        ),
        ("post", "/auth/login", "Login", "LoginCredentials", false),
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
            "get",
            "/me/purchases/{product_id}",
            "Whether the user already owns or has an open order",
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
            "Submit payment receipt",
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
    doc["components"]["schemas"]["TokenInput"] =
        json!({"type":"object","required":["token"],"properties":{"token":{"type":"string"}}});
    doc["components"]["schemas"]["EmailInput"] = json!({"type":"object","required":["email"],"properties":{"email":{"type":"string","format":"email"}}});
    doc["components"]["schemas"]["ResetInput"] = json!({"type":"object","required":["token","password"],"properties":{"token":{"type":"string"},"password":{"type":"string","minLength":12,"maxLength":128}}});
    doc["components"]["schemas"]["LoginCredentials"] = json!({"type":"object","required":["email","password"],"properties":{"email":{"type":"string","format":"email"},"password":{"type":"string"},"totp_code":{"type":"string","description":"管理员 6 位 TOTP 动态码或单次恢复码"}}});
    for (method, path, title, schema, auth) in routes {
        let mut op = json!({"summary":title,"responses":{"200":{"description":"Success","content":{"application/json":{"schema":{"$ref":"#/components/schemas/Envelope"}}}},"400":{"description":"Malformed request"},"401":{"description":"Unauthenticated or expired session"},"403":{"description":"CSRF, role, or entitlement denied"},"404":{"description":"Missing or owned by another user"},"409":{"description":"State/idempotency/reference conflict"},"413":{"description":"Upload too large"},"422":{"description":"Validation failed"},"429":{"description":"Rate limited"},"500":{"description":"Transaction failed"},"503":{"description":"Dependency/payment configuration unavailable"}},"parameters":[]});
        if path == "/auth/register" {
            op["responses"]["202"] =
                json!({"description":"Registration accepted; verification email may follow"});
        }
        if method == "post"
            && !path.ends_with("/refund")
            && !path.ends_with("/accept")
            && !path.ends_with("/messages")
            && !path.ends_with("/approve")
            && !path.ends_with("/reject")
            && !path.ends_with("/cancel")
            && !path.starts_with("/auth/login")
            && !path.starts_with("/auth/register")
            && !path.starts_with("/auth/verify-email")
            && !path.starts_with("/auth/password/")
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
            let mime = if matches!(schema, "Receipt" | "DigitalAsset") {
                "multipart/form-data"
            } else {
                "application/json"
            };
            op["requestBody"] = json!({"required":true,"content":{mime:{"schema":{"$ref":format!("#/components/schemas/{schema}")}}}});
            if method == "patch" && schema != "BlogInput" {
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
        if path == "/me/downloads/{id}" {
            op["responses"]["200"]["content"] = json!({"application/pdf":{"schema":{"type":"string","format":"binary"}},"application/zip":{"schema":{"type":"string","format":"binary"}}});
        }
        op["parameters"] = json!(params);
        doc["paths"][path][method] = op;
    }
    axum::Json(doc)
}

#[cfg(test)]
mod tests {
    #[tokio::test]
    async fn delivery_and_blog_operations_are_documented_with_authentication() {
        let axum::Json(doc) = super::document().await;
        for (method, path) in [
            ("post", "/admin/products/{id}/file"),
            ("get", "/me/downloads/{id}"),
            ("post", "/admin/orders/{id}/refund"),
            ("get", "/me/tickets"),
            ("get", "/me/tickets/{id}"),
            ("post", "/me/tickets/{id}/messages"),
            ("post", "/me/tickets/{id}/accept"),
            ("get", "/admin/tickets"),
            ("get", "/admin/tickets/{id}"),
            ("post", "/admin/tickets/{id}/messages"),
            ("get", "/admin/blog"),
            ("post", "/admin/blog"),
            ("get", "/admin/blog/{id}"),
            ("patch", "/admin/blog/{id}"),
        ] {
            let operation = &doc["paths"][path][method];
            assert!(operation.is_object(), "missing {method} {path}");
            assert_eq!(operation["security"], serde_json::json!([{"session": []}]));
            if method != "get" {
                assert!(
                    operation["parameters"]
                        .as_array()
                        .unwrap()
                        .iter()
                        .any(|parameter| parameter["name"] == "X-CSRF-Token"
                            && parameter["required"] == true)
                );
            }
        }
        for path in ["/blog", "/blog/{slug}"] {
            assert!(doc["paths"][path]["get"].is_object());
            assert!(doc["paths"][path]["get"]["security"].is_null());
        }
    }

    #[tokio::test]
    async fn file_upload_and_download_describe_binary_payloads() {
        let axum::Json(doc) = super::document().await;
        assert_eq!(
            doc["paths"]["/admin/products/{id}/file"]["post"]["requestBody"]["content"]["multipart/form-data"]
                ["schema"]["$ref"],
            "#/components/schemas/DigitalAsset"
        );
        for mime in ["application/pdf", "application/zip"] {
            assert_eq!(
                doc["paths"]["/me/downloads/{id}"]["get"]["responses"]["200"]["content"][mime]["schema"]
                    ["format"],
                "binary"
            );
        }
    }
}
