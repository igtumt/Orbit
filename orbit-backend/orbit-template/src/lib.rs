static UI_SCHEMA: &str = r#"{
  "isim": "Alan Hesaplayıcı (Dinamik)",
  "bilesenler": [
    { "tip": "sayi_girisi", "id": "genislik", "etiket": "Genişlik (m):" },
    { "tip": "sayi_girisi", "id": "yukseklik", "etiket": "Yükseklik (m):" },
    { 
      "tip": "buton", 
      "id": "hesap_btn", 
      "etiket": "📐 Alanı Hesapla", 
      "fonksiyon": "alan_bul", 
      "girdiler": ["genislik", "yukseklik"] 
    }
  ]
}"#;

#[unsafe(no_mangle)]
pub extern "C" fn get_schema_ptr() -> *const u8 { UI_SCHEMA.as_ptr() }

#[unsafe(no_mangle)]
pub extern "C" fn get_schema_len() -> usize { UI_SCHEMA.len() }

#[unsafe(no_mangle)]
pub extern "C" fn alan_bul(a: i32, b: i32) -> i32 { a * b }
