use axum::{
    routing::{get, post},
    Router,
    http::StatusCode,
    response::IntoResponse,
};
use tower_http::cors::{CorsLayer, Any};
use std::process::Command;
use std::fs;
use std::env;

#[tokio::main]
async fn main() {
    let cors = CorsLayer::new().allow_origin(Any).allow_methods(Any).allow_headers(Any);
    let app = Router::new()
        .route("/", get(|| async { "Orbit Backend Aktif! 🚀" }))
        .route("/compile", post(compile_code))
        .route("/github", post(compile_github)) 
        .layer(cors);

    let listener = tokio::net::TcpListener::bind("127.0.0.1:3000").await.unwrap();
    println!("📡 Orbit Backend çalışıyor: http://127.0.0.1:3000");
    axum::serve(listener, app).await.unwrap();
}

fn is_code_safe(code: &str) -> Result<(), String> {
    let blacklisted_terms = [
        "std::fs", "std::process", "std::net", "std::env", "Command::", "File::", "macro_rules!"
    ];
    for term in blacklisted_terms {
        if code.contains(term) {
            return Err(format!("Güvenlik İhlali! Yasaklı terim tespit edildi: {}", term));
        }
    }
    Ok(())
}

// =========================================================================
// GERÇEK YAPAY ZEKA BAĞLANTISI (GEMINI API)
// Defansif Programlama eklendi: AI dizi gönderse bile zorla nesneye çevrilir.
// =========================================================================
async fn ai_schema_injector(raw_code: &str) -> String {
    if raw_code.contains("UI_SCHEMA") { 
        return raw_code.to_string(); 
    }
    
    println!("🤖 GERÇEK YAPAY ZEKA DEVREDE: Kod analiz ediliyor...");

    // 🔴 BURAYA KENDİ ALDIĞIN API ANAHTARINI YAPIŞTIR 🔴
    let api_key = "Write your own API!"; 
    
    let url = format!("https://generativelanguage.googleapis.com/v1beta/models/gemini-2.5-flash:generateContent?key={}", api_key);

    let prompt = format!(
        "Sen usta bir yazılım mimarısın. Aşağıdaki Rust kodunu incele ve SADECE GEÇERLİ BİR JSON nesnesi döndür.
        Markdown (```json), açıklama veya fazladan hiçbir kelime KULLANMA.
        JSON formatı KESİNLİKLE şu yapıda bir 'nesne' (object) olmalıdır. ASLA doğrudan dizi (array) ile başlama!
        {{
            \"isim\": \"AI Destekli Uygulama\",
            \"bilesenler\": [
                {{ \"tip\": \"sayi_girisi\", \"id\": \"degisken_adi\", \"etiket\": \"Kullanıcıya Gösterilecek Metin\" }},
                {{ \"tip\": \"buton\", \"id\": \"calistir_btn\", \"etiket\": \"Çalıştır\", \"fonksiyon\": \"asil_fonksiyon_ismi\", \"girdiler\": [\"degisken_adi\"] }}
            ]
        }}
        
        İşte Kod:\n{}", raw_code
    );

    let client = reqwest::Client::new();
    let payload = serde_json::json!({
        "contents": [{ "parts": [{"text": prompt}] }]
    });

    let response = client.post(&url).json(&payload).send().await;

    if let Ok(res) = response {
        if let Ok(json_body) = res.json::<serde_json::Value>().await {
            let generated_text = json_body["candidates"][0]["content"]["parts"][0]["text"].as_str().unwrap_or("").to_string();
            
            // 1. ADIM: Gelen metnin içindeki JSON yapısını ({ veya [) güvenli bir şekilde bul
            let start_idx = generated_text.find(|c| c == '{' || c == '[').unwrap_or(0);
            let end_idx = generated_text.rfind(|c| c == '}' || c == ']').map(|i| i + 1).unwrap_or(generated_text.len());
            
            let clean_json = if start_idx < end_idx {
                generated_text[start_idx..end_idx].trim().to_string()
            } else {
                generated_text.trim().to_string()
            };

            // 2. ADIM: DEFANSİF PROGRAMLAMA
            // AI inat edip sadece bir dizi ('[') gönderdiyse, onu bizim istediğimiz formata zorla oturtuyoruz.
            let final_json = if clean_json.starts_with('[') {
                format!("{{\"isim\": \"✨ AI Tarafından Üretildi\", \"bilesenler\": {}}}", clean_json)
            } else {
                clean_json
            };

            println!("✨ AI Şeması Başarıyla Filtrelendi ve Onaylandı:\n{}", final_json);

            let wrapped_user_code = format!(
                "
static UI_SCHEMA: &str = r#\"{}\"#;

#[unsafe(no_mangle)]
pub extern \"C\" fn get_schema_ptr() -> *const u8 {{ UI_SCHEMA.as_ptr() }}

#[unsafe(no_mangle)]
pub extern \"C\" fn get_schema_len() -> usize {{ UI_SCHEMA.len() }}

// --- KULLANICININ ORİJİNAL KODU ---
{}
",
                final_json, raw_code
            );
            return wrapped_user_code;
        }
    }

    println!("❌ Yapay Zeka bağlantısı koptu, orijinal kod geri dönülüyor.");
    raw_code.to_string()
}

async fn compile_code(code: String) -> impl IntoResponse {
    if let Err(security_msg) = is_code_safe(&code) {
        println!("🚨 GÜVENLİK UYARISI: {}", security_msg);
        return (StatusCode::FORBIDDEN, security_msg.into_bytes());
    }

    // Kodu AI'dan geçiriyoruz
    let smart_code = ai_schema_injector(&code).await;

    let current_dir = env::current_dir().unwrap();
    let template_path = current_dir.join("orbit-template");
    let template_path_str = template_path.to_str().unwrap();

    let lib_path = template_path.join("src/lib.rs");
    let _ = fs::write(&lib_path, smart_code);

    println!("🔒 KOD DOCKER KASASINA GÖNDERİLİYOR...");

    let output = Command::new("docker")
        .arg("run")
        .arg("--rm")
        .arg("-v")
        .arg(format!("{}:/app", template_path_str))
        .arg("-w")
        .arg("/app")
        .arg("rust:latest")
        .arg("sh")
        .arg("-c")
        .arg("rustup target add wasm32-unknown-unknown && cargo build --target=wasm32-unknown-unknown --release")
        .output()
        .expect("Docker komutu çalıştırılamadı! Docker Desktop açık mı?");

    if output.status.success() {
        println!("✅ Docker derlemesi başarılı!");
        let wasm_path = template_path.join("target/wasm32-unknown-unknown/release/orbit_template.wasm");
        let wasm_bytes = fs::read(wasm_path).unwrap();
        (StatusCode::OK, wasm_bytes)
    } else {
        println!("❌ Docker derleme hatası!");
        let err_msg = String::from_utf8(output.stderr).unwrap();
        (StatusCode::BAD_REQUEST, err_msg.into_bytes())
    }
}

async fn compile_github(repo_url: String) -> impl IntoResponse {
    let repo_url = repo_url.trim();
    println!("🐙 GitHub'dan proje çekiliyor: {}", repo_url);

    let current_dir = env::current_dir().unwrap();
    let workspace = current_dir.join("github-workspace");
    let workspace_str = workspace.to_str().unwrap();

    let _ = fs::remove_dir_all(&workspace);
    let clone_output = Command::new("git").arg("clone").arg(repo_url).arg(&workspace).output().unwrap();

    if !clone_output.status.success() {
        return (StatusCode::BAD_REQUEST, "GitHub reposu klonlanamadı.".to_string().into_bytes());
    }

    if let Err(sec_err) = scan_directory_for_threats(workspace_str) {
        println!("🚨 GITHUB GÜVENLİK UYARISI: {}", sec_err);
        let _ = fs::remove_dir_all(&workspace);
        return (StatusCode::FORBIDDEN, sec_err.into_bytes());
    }

    println!("🔒 GITHUB PROJESİ DOCKER KASASINDA DERLENİYOR...");

    let build_output = Command::new("docker")
        .arg("run")
        .arg("--rm")
        .arg("-v")
        .arg(format!("{}:/app", workspace_str))
        .arg("-w")
        .arg("/app")
        .arg("rust:latest")
        .arg("sh")
        .arg("-c")
        .arg("rustup target add wasm32-unknown-unknown && cargo build --target=wasm32-unknown-unknown --release")
        .output()
        .expect("Docker komutu çalıştırılamadı!");

    if build_output.status.success() {
        let release_dir = workspace.join("target/wasm32-unknown-unknown/release");
        if let Ok(entries) = fs::read_dir(&release_dir) {
            for entry in entries.flatten() {
                let path = entry.path();
                if path.extension().and_then(|s| s.to_str()) == Some("wasm") {
                    println!("🎉 Docker'dan WASM dosyası çıkarıldı.");
                    let wasm_bytes = fs::read(path).unwrap();
                    return (StatusCode::OK, wasm_bytes);
                }
            }
        }
        (StatusCode::INTERNAL_SERVER_ERROR, "WASM dosyası bulunamadı!".to_string().into_bytes())
    } else {
        println!("❌ Docker derleme hatası!");
        let err_msg = String::from_utf8(build_output.stderr).unwrap();
        (StatusCode::BAD_REQUEST, err_msg.into_bytes())
    }
}

fn scan_directory_for_threats(dir_path: &str) -> Result<(), String> {
    for entry in walkdir::WalkDir::new(dir_path).into_iter().filter_map(|e| e.ok()) {
        if entry.path().extension().and_then(|s| s.to_str()) == Some("rs") {
            if let Ok(content) = fs::read_to_string(entry.path()) {
                is_code_safe(&content)?;
            }
        }
    }
    Ok(())
}
