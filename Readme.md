# S3 Manager for Robinbrick
Must declare these variables before using this library:
- AWS_ACCESS_KEY_ID
- AWS_SECRET_ACCESS_KEY
- AWS_REGION

```rust
// Get an instance of client first (must have all env vars defined above)
use service::start_s3_aws_connection;
use upload_image_in_base64;

let client = start_s3_aws_connection().await;

let result: Result<String, _> = upload_image_in_base64(&client, "base64 string goes here", Some("image.png")).await;
// or with a random name
let result: Result<String, _> = upload_image_in_base64(&client, "base64 string goes here", None).await;
```