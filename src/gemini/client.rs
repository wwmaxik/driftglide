use std::path::Path;
use std::process::Command;
use tracing::info;

/// Простое и надежное кодирование base64 без внешних зависимостей
pub fn base64_encode(data: &[u8]) -> String {
    const TABLE: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut out = String::with_capacity((data.len() + 2) / 3 * 4);
    for chunk in data.chunks(3) {
        let b0 = chunk[0] as usize;
        let b1 = if chunk.len() > 1 { chunk[1] as usize } else { 0 };
        let b2 = if chunk.len() > 2 { chunk[2] as usize } else { 0 };
        let triple = (b0 << 16) | (b1 << 8) | b2;
        out.push(TABLE[(triple >> 18) & 0x3F] as char);
        out.push(TABLE[(triple >> 12) & 0x3F] as char);
        if chunk.len() > 1 {
            out.push(TABLE[(triple >> 6) & 0x3F] as char);
        } else {
            out.push('=');
        }
        if chunk.len() > 2 {
            out.push(TABLE[triple & 0x3F] as char);
        } else {
            out.push('=');
        }
    }
    out
}

#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
pub struct ChatTurn {
    pub role: String, // "user" or "model"
    pub text: String,
}

/// Вызов Gemini API через curl в фоновом потоке
pub fn call_gemini(
    api_key: &str,
    model: &str,
    history: &[ChatTurn],
    image_path: Option<&Path>,
) -> Result<String, String> {
    let key = api_key.trim();
    if key.is_empty() {
        return Err("API ключ Gemini не настроен. Нажмите на шестерёнку в окне и введите ключ.".into());
    }

    if history.is_empty() {
        return Err("Пустой запрос.".into());
    }

    let url = format!(
        "https://generativelanguage.googleapis.com/v1beta/models/{}:generateContent?key={}",
        model, key
    );

    let mut contents = Vec::new();
    let mut image_attached = false;

    for turn in history {
        let mut parts = Vec::new();

        // Прикрепляем изображение к первому сообщению пользователя
        if turn.role == "user" && !image_attached {
            if let Some(path) = image_path {
                if let Ok(bytes) = std::fs::read(path) {
                    let b64 = base64_encode(&bytes);
                    parts.push(serde_json::json!({
                        "inline_data": {
                            "mime_type": "image/png",
                            "data": b64
                        }
                    }));
                    info!("Кроп изображения добавлен к первому сообщению пользователя ({} байт)", bytes.len());
                }
            }
            image_attached = true;
        }

        parts.push(serde_json::json!({
            "text": turn.text
        }));

        contents.push(serde_json::json!({
            "role": turn.role,
            "parts": parts
        }));
    }

    let payload = serde_json::json!({
        "contents": contents
    });

    let payload_str = serde_json::to_string(&payload)
        .map_err(|e| format!("Ошибка сериализации запроса: {}", e))?;

    info!("Отправка запроса к Gemini ({}, сообщений в истории: {})", model, history.len());

    let mut child = Command::new("curl")
        .arg("-s")
        .arg("--connect-timeout")
        .arg("10")
        .arg("--max-time")
        .arg("60")
        .arg("-X")
        .arg("POST")
        .arg(&url)
        .arg("-H")
        .arg("Content-Type: application/json")
        .arg("--data-binary")
        .arg("@-")
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .spawn()
        .map_err(|e| format!("Не удалось запустить curl: {}", e))?;

    if let Some(mut stdin) = child.stdin.take() {
        use std::io::Write;
        stdin.write_all(payload_str.as_bytes())
            .map_err(|e| format!("Не удалось передать данные в curl: {}", e))?;
    }

    let output = child.wait_with_output()
        .map_err(|e| format!("Ошибка ожидания ответа curl: {}", e))?;

    if !output.status.success() {
        return Err(format!("Ошибка curl: статус {:?}", output.status.code()));
    }

    let resp_str = String::from_utf8_lossy(&output.stdout);
    let resp_json: serde_json::Value = serde_json::from_str(&resp_str)
        .map_err(|e| format!("Ошибка разбора ответа: {}\n{}", e, resp_str))?;

    if let Some(err_obj) = resp_json.get("error") {
        let msg = err_obj["message"].as_str().unwrap_or("Неизвестная ошибка API");
        return Err(format!("Gemini API Error: {}", msg));
    }

    if let Some(candidates) = resp_json["candidates"].as_array() {
        if let Some(first) = candidates.first() {
            if let Some(parts) = first["content"]["parts"].as_array() {
                let mut text = String::new();
                for p in parts {
                    if let Some(t) = p["text"].as_str() {
                        text.push_str(t);
                    }
                }
                if !text.is_empty() {
                    return Ok(text.trim().to_string());
                }
            }
        }
    }

    Err(format!("Пустой ответ от Gemini: {}", resp_str))
}
