use std::{fs, str::FromStr};

#[allow(unused)]
use aws_sdk_s3::{
    error::PutObjectError,
    types::{ByteStream, SdkError},
    Client,
};

use css_color_parser::Color;
use dotenv::dotenv;
use image::GenericImageView;
use image_base64::to_base64;

pub const DEFAULT_BUCKET_NAME: &str = "images-robinbrick";
pub const BASE_BUCKET_URL: &str = "https://images-robinbrick.s3.eu-west-1.amazonaws.com/";

pub const BACKGROUND_IMAGE_WIDTH: u32 = 1400;
pub const BACKGROUND_IMAGE_HEIGHT: u32 = 400;
pub const MAX_LOGO_WIDTH: u32 = 1000;
pub const MAX_LOGO_HEIGHT: u32 = 300;
/// Env variable names that must be in here for this library to work
const ALL_VARS: [&str; 3] = ["AWS_ACCESS_KEY_ID", "AWS_SECRET_ACCESS_KEY", "AWS_REGION"];

#[derive(Debug, Clone, Default, PartialEq, Eq, PartialOrd, Ord)]
pub struct EncodedImage<'a> {
    /// Bytes in a base64 str
    pub bytes: &'a str,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, PartialOrd, Ord)]
pub enum S3Error {
    #[default]
    ColorParseError,
    ImageDecodeError,
    ImageEncodeError,
    NotFoundError,
}

/// Entry point of this library.
/// Make sure to have all the credentials defined in the .env file or environment variables before calling this method.
/// Otherwise, it Will panic.
#[allow(unused)]
pub async fn start_s3_aws_connection() -> Client {
    dotenv().ok();
    for var in ALL_VARS {
        match dotenv::var(var) {
            Ok(_) => {},
            Err(_) => panic!("Env variable: {var} not found in your environment. You must have these variables: {:?}. in order to use this library", ALL_VARS),
        }
    }
    let config = aws_config::load_from_env().await;
    aws_sdk_s3::Client::new(&config)
}

/// Call service::start_s3_aws_connection().await first to get the client
/// Uploads an image to AWS s3 bucket and returns the URL to the publicly accessible image
/// Pass a None value to the file_name_opt to let it get assigned a random number as the name,
/// Or pass a name and an extension to make it use that.
pub async fn upload_image_in_base64<'a>(
    client: &Client,
    image: EncodedImage<'a>,
    file_name_opt: Option<&str>,
) -> Result<String, SdkError<PutObjectError>> {
    let conversion_tuple = decode_base64_to_image(image);
    let body = ByteStream::from(conversion_tuple.0);

    let file_name = match file_name_opt {
        Some(file_name) => file_name,
        None => conversion_tuple.1.as_str(),
    };

    match client
        .put_object()
        .bucket(DEFAULT_BUCKET_NAME)
        .key(file_name)
        .body(body)
        .set_grant_read(
            Some("uri=http://acs.amazonaws.com/groups/global/AllUsers".to_string()), // grant read access to everyone
        )
        .send()
        .await
    {
        Ok(_) => Ok(get_image_url_from_image_name(file_name)),
        Err(e) => Err(e),
    }
}

/// Join the base bucket url and the image name
pub fn get_image_url_from_image_name(image_name: &str) -> String {
    format!("{BASE_BUCKET_URL}{image_name}")
}

/// Converts base64str to byte vec and a filename
pub fn decode_base64_to_image(image: EncodedImage) -> (Vec<u8>, String) {
    let image_str = image.bytes;
    let extension = extract_extension_from_base64_metadata(image_str);
    let file_name = rand::random::<u64>();
    (
        image_base64::from_base64(image.bytes.to_string()),
        format!("{file_name}.{extension}"),
    )
}
/// This is to extact this: "png" from: "data:image/png;base64"
fn extract_extension_from_base64_metadata(base64: &str) -> &str {
    let error_msg = "Error inside the s3 library inside the extract_extension_from_base64_metadata function. Explanation: Base64 string doesn't fit the format the creator of this library thought it did. Specifically, the start. ";
    let first = base64.split(";").collect::<Vec<&str>>();
    let second = first
        .get(0)
        .expect(error_msg)
        .split("/")
        .collect::<Vec<&str>>();
    return second.get(1).expect(error_msg);
}

/// Changes the background color of the image (Also the most repeat colors)
/// Always returns a JPG
pub fn change_background<'a>(
    encoded_image: EncodedImage<'a>,
    color_hex: &str,
) -> Result<String, S3Error> {
    // Parse color
    let color = match Color::from_str(color_hex) {
        Ok(color) => color,
        Err(e) => {
            println!("{}", e.to_string());
            return Err(S3Error::ColorParseError);
        }
    };

    // Decode base64 into Vec<u8> & image name
    let decoded_image = decode_base64_to_image(encoded_image);
    let mut logo = match image::load_from_memory(&decoded_image.0) {
        Ok(image) => image,
        Err(_) => return Err(S3Error::ImageDecodeError),
    };
    logo = logo.resize(
        MAX_LOGO_WIDTH,
        MAX_LOGO_HEIGHT,
        image::imageops::FilterType::Nearest,
    );
    let logo_dimensions = logo.dimensions();

    let width_start = 1400 / 2 - logo_dimensions.0 / 2;
    let height_start = 400 / 2 - logo_dimensions.1 / 2;

    let mut background = image::ImageBuffer::new(1400, 400);
    // Iterate over the coordinates and pixels of the image
    for (x, y, pixel) in background.enumerate_pixels_mut() {

        // bg size is 1400x400. Logo size will be made into 1000x300
        // To fit proportionally, logo should start at (201, 51) and end at (1000, 300)
        if x >= width_start
            && y >= height_start
            && x < width_start + logo_dimensions.0
            && y < height_start + logo_dimensions.1
        {
            // Write image
            // convert bg x & y to logo x & y
            let pixel_to_write = logo.get_pixel(x - width_start, y - height_start);
            if pixel_to_write.0[3] >= 200 {
                *pixel = pixel_to_write;
            } else {
                *pixel = image::Rgba([color.r, color.g, color.b as u8, 255]);
            }
        } else {
            *pixel = image::Rgba([color.r, color.g, color.b as u8, 255]);
        }
    }
    let file_name = format!("{}.jpeg", rand::random::<u64>());
    match background.save_with_format(file_name.clone(), image::ImageFormat::Jpeg) {
        Ok(_) => {}
        Err(_) => return Err(S3Error::ImageEncodeError),
    };
    let image_buf = match image::open(file_name.as_str()) {
        Ok(_) => to_base64(file_name.as_str()),
        Err(_) => return Err(S3Error::NotFoundError),
    };
    match fs::remove_file(file_name) {
        Ok(_) => {}
        Err(_) => {}
    };
    Ok(image_buf)
}

#[cfg(test)]
mod tests {
    use std::{fs, path::Path};

    use aws_sdk_s3::types::ByteStream;

    use crate::{
        change_background, start_s3_aws_connection, upload_image_in_base64, EncodedImage,
        BASE_BUCKET_URL,
    };

    #[test]
    fn change_background_test() {
        let base64_str = include_str!("../testimage.txt");
        assert!(matches!(
            change_background(EncodedImage { bytes: base64_str }, "#ff0"),
            Ok(_)
        ));
    }
    /// Grabs a test image string in base64 and turns it into a png
    #[test]
    fn decode_base64_to_image() {
        let base64_str = include_str!("../testimage.txt");

        let image_bytes = image_base64::from_base64(base64_str.to_string());
        fs::write("path.png", image_bytes).unwrap();
    }
    /// Connects to aws and attempts to list buckets
    #[tokio::test]
    async fn connect_to_aws() {
        let client = start_s3_aws_connection().await;
        let buckets = client.list_buckets().send().await;
        assert!(
            matches!(buckets, Ok(_)),
            "Buckets could not be obtained from aws, Reason: {:#?}",
            buckets
        );
    }

    /// Checks that the bucket named images-robinbrick exists and is available
    /// This is only for robinbrick internal use.
    #[tokio::test]
    async fn test_that_images_bucket_exists() {
        let bucket_name = "images-robinbrick";

        let client = start_s3_aws_connection().await;
        let bucket_res = client.list_buckets().send().await;
        assert!(
            matches!(bucket_res, Ok(_)),
            "Buckets could not be obtained from aws, Reason: {:#?}",
            bucket_res
        );

        let buckets = bucket_res.unwrap();
        for bucket in buckets.buckets().unwrap() {
            if bucket.name().unwrap() == bucket_name {
                return;
            }
        }
        panic!("No buckets named {bucket_name} found");
    }

    /// Attempts to get and print all the objects inside a bucket
    /// This is only for robinbrick internal use. (This test is to be disabled, as it's only for demonstration purposes)
    #[tokio::test]
    async fn get_all_objects_from_bucket() {
        let bucket_name = "images-robinbrick";

        let client = start_s3_aws_connection().await;
        let bucket_res = client.list_buckets().send().await;
        assert!(
            matches!(bucket_res, Ok(_)),
            "Buckets could not be obtained from aws, Reason: {:#?}",
            bucket_res
        );

        let buckets = bucket_res.unwrap();
        let bucket = buckets
            .buckets()
            .unwrap()
            .iter()
            .find(|bucket| bucket.name() == Some(bucket_name))
            .unwrap();

        let objects = client
            .list_objects_v2()
            .bucket(bucket.name().unwrap())
            .send()
            .await;
        println!("{:#?}", objects);
    }

    #[tokio::test]
    async fn upload_png_to_bucket_and_get_back_url() {
        let bucket_name = "images-robinbrick";
        let file_name = "aaaa.png";
        let client = start_s3_aws_connection().await;
        let bucket_res = client.list_buckets().send().await;
        assert!(
            matches!(bucket_res, Ok(_)),
            "Buckets could not be obtained from aws, Reason: {:#?}",
            bucket_res
        );

        let body = ByteStream::from_path(Path::new("path.png")).await;
        let _output = client
            .put_object()
            .bucket(bucket_name)
            .key(file_name)
            .body(body.unwrap())
            .set_grant_read(
                Some("uri=http://acs.amazonaws.com/groups/global/AllUsers".to_string()), // grant read access to everyone
            )
            .send()
            .await;
        println!("{}{}", crate::BASE_BUCKET_URL, file_name);
    }
    /// Tests the upload method that this whole library is about.
    #[tokio::test]
    async fn test_upload_both() {
        // Get a client first
        let client = start_s3_aws_connection().await;
        let image_in_base64 = include_str!("../testimage.txt");
        let image_with_bg = change_background(
            EncodedImage {
                bytes: image_in_base64,
            },
            "#fff",
        )
        .unwrap();
        let result1 = upload_image_in_base64(
            &client,
            EncodedImage {
                bytes: image_in_base64,
            },
            None,
        )
        .await;
        let result2 = upload_image_in_base64(
            &client,
            EncodedImage {
                bytes: image_with_bg.as_str(),
            },
            None,
        )
        .await;
        println!("{:#?}", result1);
        println!("{:#?}", result2);
        assert!(matches!(result1, Ok(_)) && matches!(result2, Ok(_)));
        assert!(
            result1.unwrap().starts_with(BASE_BUCKET_URL)
                && result2.unwrap().starts_with(BASE_BUCKET_URL)
        );
    }
    /// Tests the upload method that this whole library is about. (Named)
    #[tokio::test]
    async fn test_upload_named() {
        // Get a client first
        let client = start_s3_aws_connection().await;
        let image_in_base64 = include_str!("../testimage.txt");
        let result = upload_image_in_base64(
            &client,
            EncodedImage {
                bytes: image_in_base64,
            },
            Some("testimage12345687.png"),
        )
        .await;
        println!("{:#?}", result);
        assert!(matches!(result, Ok(_)));
        assert_eq!(
            result.unwrap(),
            format!("{BASE_BUCKET_URL}testimage12345687.png")
        );
    }
}
