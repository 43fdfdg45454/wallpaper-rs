use super::{Server, Who, cookie};
use reqwest::header::COOKIE;
use reqwest::{Method, RequestBuilder, Response, StatusCode};
use serde_json::{Value, json};

impl Server {
    pub fn call(&self, method: Method, path: &str, who: &Who) -> RequestBuilder {
        let request = self.http.request(method, format!("{}{path}", self.url));
        match who {
            Who::Nobody => request,
            Who::Cookie(value) => request.header(COOKIE, format!("wprs={value}")),
            Who::Bearer(token) => request.bearer_auth(token),
        }
    }

    pub async fn json(
        &self,
        method: Method,
        path: &str,
        who: &Who,
        body: Option<Value>,
    ) -> (StatusCode, Value) {
        let request = self.call(method, path, who);
        let request = match body {
            Some(body) => request.json(&body),
            None => request,
        };
        let response = request.send().await.unwrap();
        let status = response.status();
        (status, response.json().await.unwrap_or(Value::Null))
    }

    pub async fn form(&self, path: &str, who: &Who, fields: &[(&str, &str)]) -> Response {
        self.call(Method::POST, path, who).form(fields).send().await.unwrap()
    }

    pub async fn admin(&self) -> Who {
        let token = self.app.setup_token().await.unwrap();
        let response = self
            .form(
                "/setup",
                &Who::Nobody,
                &[("token", &token), ("handle", "admin"), ("password", "correct horse battery")],
            )
            .await;
        assert_eq!(response.status(), StatusCode::SEE_OTHER);
        Who::Cookie(cookie(&response))
    }

    pub async fn user(&self, admin: &Who, handle: &str, role: &str) -> Who {
        let (status, invite) =
            self.json(Method::POST, "/api/v1/admin/invites", admin, Some(json!({"role": role}))).await;
        assert_eq!(status, StatusCode::CREATED, "{invite}");
        let body = json!({"handle": handle, "password": "a long enough password", "invite": invite["code"]});
        assert_eq!(
            self.json(Method::POST, "/api/v1/auth/register", &Who::Nobody, Some(body)).await.0,
            StatusCode::CREATED
        );
        let response = self
            .form("/login", &Who::Nobody, &[("handle", handle), ("password", "a long enough password")])
            .await;
        assert_eq!(response.status(), StatusCode::SEE_OTHER);
        Who::Cookie(cookie(&response))
    }

    pub async fn device(&self, handle: &str) -> (Who, String) {
        let body =
            json!({"handle": handle, "password": "a long enough password", "device": {"platform": "linux"}});
        let (status, device) = self.json(Method::POST, "/api/v1/auth/login", &Who::Nobody, Some(body)).await;
        assert_eq!(status, StatusCode::OK, "{device}");
        (Who::Bearer(device["token"].as_str().unwrap().into()), device["device_id"].as_str().unwrap().into())
    }
}
