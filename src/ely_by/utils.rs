use serde::Deserialize;

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ElyByUserInfo {
    uuid: String,
    username: String,
    profile_link: String,
}

pub async fn get_user_info(token: &str) -> anyhow::Result<ElyByUserInfo> {
    let client = reqwest::Client::new();
    let user_info = client.get("https://account.ely.by/api/account/v1/info")
        .header("Authorization", format!("Bearer {}", token))
        .send()
        .await?
        .json::<ElyByUserInfo>()
        .await?;
    Ok(user_info)
}
