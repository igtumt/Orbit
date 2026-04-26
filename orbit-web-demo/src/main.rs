use yew::prelude::*;
use web_sys::{Event, MouseEvent, HtmlCanvasElement, CanvasRenderingContext2d, ImageData, HtmlTextAreaElement, HtmlInputElement, InputEvent};
use wasm_bindgen::prelude::*;
use wasm_bindgen_futures::{spawn_local, JsFuture};
use js_sys::{WebAssembly, Function, Reflect, Object};
use wasm_bindgen::JsCast;
use gloo_timers::callback::Interval;
use gloo_net::http::Request;
use serde::{Deserialize, Serialize};

// =========================================================================
// 1. JSON ŞEMASI İÇİN RUST YAPILARI (SERDE)
// =========================================================================
#[derive(Deserialize, Serialize, Clone, PartialEq)]
struct UIComponent {
    tip: String,   
    id: String,    
    etiket: String, 
    fonksiyon: Option<String>, 
    girdiler: Option<Vec<String>>, 
}

#[derive(Deserialize, Serialize, Clone, PartialEq)]
struct UISchema {
    isim: String,
    bilesenler: Vec<UIComponent>,
}

// =========================================================================
// 2. UYGULAMA TİPLERİ VE VERİ YAPISI
// =========================================================================
#[derive(Clone, PartialEq)]
enum AppMode {
    Graphics,
    Calculator,
    UnknownTerminal,
    Dynamic(UISchema), 
}

#[derive(Clone)]
struct AppData {
    id: usize,
    mode: AppMode,
    instance: WebAssembly::Instance,
}

impl PartialEq for AppData {
    fn eq(&self, other: &Self) -> bool { self.id == other.id }
}

#[derive(Properties, PartialEq)]
struct WindowProps {
    pub app: AppData,
}

// =========================================================================
// 3. YENİ: SÜRÜKLENEBİLİR VE KAPATILABİLİR PENCERE ÇERÇEVESİ (WINDOW MANAGER)
// =========================================================================
#[derive(Properties, PartialEq)]
struct WindowFrameProps {
    pub id: usize,
    pub title: String,
    pub on_close: Callback<usize>,
    pub children: Children,
}

#[function_component(WindowFrame)]
fn window_frame(props: &WindowFrameProps) -> Html {
    // Pencerelerin başlangıç konumları üst üste binmesin diye ID'ye göre hafifçe kaydırıyoruz
    let initial_offset = (props.id as i32 * 40) % 300;
    let x = use_state(|| 50 + initial_offset); 
    let y = use_state(|| 50 + initial_offset);
    let is_dragging = use_state(|| false);

    // Sürükleme Başlangıcı
    let on_mouse_down = { let is_dragging = is_dragging.clone(); Callback::from(move |_| is_dragging.set(true)) };
    
    // Sürükleme Bitişi
    let on_mouse_up = { let is_dragging = is_dragging.clone(); Callback::from(move |_| is_dragging.set(false)) };
    
    // Sürükleme Hareketi (Sadece is_dragging true ise çalışır)
    let on_mouse_move = {
        let is_dragging = is_dragging.clone();
        let x = x.clone();
        let y = y.clone();
        Callback::from(move |e: MouseEvent| {
            if *is_dragging {
                x.set(*x + e.movement_x()); // Farenin X hareketini ekle
                y.set(*y + e.movement_y()); // Farenin Y hareketini ekle
            }
        })
    };

    // Kapatma Tuşu
    let on_close_click = {
        let on_close = props.on_close.clone();
        let id = props.id;
        Callback::from(move |_| on_close.emit(id)) // App state'ine bu ID'yi kapatmasını söyler
    };

    html! {
        <>
            // GÖRÜNMEZ YAKALAMA KATMANI (Sürüklerken farenin pencereden kaymasını önler)
            if *is_dragging {
                <div 
                    onmousemove={on_mouse_move.clone()} 
                    onmouseup={on_mouse_up.clone()}
                    style="position: fixed; top: 0; left: 0; width: 100vw; height: 100vh; z-index: 9999; cursor: grabbing;"
                ></div>
            }

            // ASIL PENCERE
            <div style={format!("position: absolute; left: {}px; top: {}px; width: 260px; background: white; border-radius: 8px; box-shadow: 0 10px 30px rgba(0,0,0,0.3); overflow: hidden; display: flex; flex-direction: column; z-index: {};", *x, *y, if *is_dragging { 10000 } else { props.id })}>
                
                // BAŞLIK ÇUBUĞU (Tut-Sürükle)
                <div 
                    onmousedown={on_mouse_down}
                    style="background: #2b303b; color: white; padding: 10px; cursor: grab; display: flex; justify_content: space-between; align_items: center; user-select: none;"
                >
                    <span style="font-weight: bold; font-size: 13px; text-overflow: ellipsis; white-space: nowrap; overflow: hidden;">{ &props.title }</span>
                    // [X] Butonu (Mac stili)
                    <button 
                        onclick={on_close_click} 
                        style="background: #ff5f56; border: none; border-radius: 50%; width: 14px; height: 14px; cursor: pointer; flex-shrink: 0;"
                        title="Pencereyi Kapat"
                    ></button>
                </div>

                // PENCERE İÇERİĞİ (Grafik, Hesap Makinesi veya Dinamik UI buraya render edilir)
                <div style="background: #fafafa;">
                    { for props.children.iter() }
                </div>
            </div>
        </>
    }
}

// =========================================================================
// 4. UYGULAMA İÇERİKLERİ (Artık çerçevesiz, direkt WindowFrame içine oturuyorlar)
// =========================================================================
#[function_component(GraphicsWindow)]
fn graphics_window(props: &WindowProps) -> Html {
    let is_playing = use_state(|| true);
    let time_counter = use_mut_ref(|| 0u32);
    let canvas_ref = use_node_ref();
    let instance = props.app.instance.clone();

    let toggle_play = { let is_playing = is_playing.clone(); Callback::from(move |_| { is_playing.set(!*is_playing); }) };
    
    let on_mouse_move = {
        let instance = instance.clone();
        Callback::from(move |e: MouseEvent| {
            let x = e.offset_x() as f32; let y = e.offset_y() as f32;
            let exports = Reflect::get(&instance, &"exports".into()).unwrap();
            if let Ok(f) = Reflect::get(&exports, &"update_mouse".into()) { 
                let func: Function = f.into(); let _ = func.call2(&JsValue::NULL, &JsValue::from(x), &JsValue::from(y)); 
            }
        })
    };

    {
        let instance = instance.clone(); let time_counter = time_counter.clone(); let canvas_ref = canvas_ref.clone();
        use_effect_with(is_playing.clone(), move |playing| {
            let interval = if **playing {
                let inst = instance.clone(); let t_counter = time_counter.clone(); let c_ref = canvas_ref.clone();
                Some(Interval::new(16, move || {
                    let exports = Reflect::get(&inst, &"exports".into()).unwrap();
                    let memory = Reflect::get(&exports, &"memory".into()).unwrap();
                    let memory_obj: WebAssembly::Memory = memory.into();
                    if let Ok(gen_obj) = Reflect::get(&exports, &"generate_frame".into()) {
                        if let Ok(ptr_obj) = Reflect::get(&exports, &"get_buffer_ptr".into()) {
                            let generate_func: Function = gen_obj.into(); let get_ptr_func: Function = ptr_obj.into();
                            let current_time = *t_counter.borrow(); *t_counter.borrow_mut() = current_time + 2; 
                            let _ = generate_func.call1(&JsValue::NULL, &JsValue::from(current_time));
                            let ptr_val = get_ptr_func.call0(&JsValue::NULL).unwrap();
                            let ptr = ptr_val.as_f64().unwrap() as u32;
                            let memory_buffer = memory_obj.buffer();
                            let clamped_array = js_sys::Uint8ClampedArray::new_with_byte_offset_and_length(&memory_buffer, ptr, 160000);
                            let image_data = ImageData::new_with_js_u8_clamped_array_and_sh(&clamped_array, 200, 200).unwrap();
                            if let Some(canvas_element) = c_ref.cast::<HtmlCanvasElement>() {
                                let ctx: CanvasRenderingContext2d = canvas_element.get_context("2d").unwrap().unwrap().dyn_into().unwrap();
                                let _ = ctx.put_image_data(&image_data, 0.0, 0.0);
                            }
                        }
                    }
                }))
            } else { None };
            move || { drop(interval); }
        });
    }

    html! {
        <div style="padding: 15px; background: #222; text-align: center;">
            <canvas ref={canvas_ref.clone()} onmousemove={on_mouse_move} width="200" height="200" style="background: #000; border: 2px solid #555; border-radius: 8px; cursor: crosshair;"></canvas>
            <br/>
            <button onclick={toggle_play} style={format!("margin-top: 10px; width: 100%; padding: 8px; color: white; border: none; border-radius: 5px; cursor: pointer; background: {};", if *is_playing { "#d32f2f" } else { "#2e7d32" })}>
                { if *is_playing { "Durdur" } else { "Başlat" } }
            </button>
        </div>
    }
}

#[function_component(CalculatorWindow)]
fn calculator_window(props: &WindowProps) -> Html {
    let num1 = use_state(|| 0);
    let num2 = use_state(|| 0);
    let result = use_state(|| None::<f64>);
    let instance = props.app.instance.clone();

    let on_num1 = { let n1 = num1.clone(); Callback::from(move |e: InputEvent| { let input: HtmlInputElement = e.target_unchecked_into(); if let Ok(v) = input.value().parse::<i32>() { n1.set(v); } }) };
    let on_num2 = { let n2 = num2.clone(); Callback::from(move |e: InputEvent| { let input: HtmlInputElement = e.target_unchecked_into(); if let Ok(v) = input.value().parse::<i32>() { n2.set(v); } }) };

    let on_calc = {
        let n1 = num1.clone(); let n2 = num2.clone(); let res = result.clone(); let inst = instance.clone();
        Callback::from(move |_| {
            let exports = Reflect::get(&inst, &"exports".into()).unwrap();
            if let Ok(func_val) = Reflect::get(&exports, &"topla".into()) {
                let func: Function = func_val.into();
                let r = func.call2(&JsValue::NULL, &JsValue::from(*n1), &JsValue::from(*n2)).unwrap();
                res.set(Some(r.as_f64().unwrap()));
            }
        })
    };

    html! {
        <div style="padding: 15px; background: #fafafa;">
            <div style="display: flex; gap: 5px; margin-bottom: 10px;">
                <input type="number" oninput={on_num1} style="width: 40%; padding: 5px;" />
                <strong style="align-content: center;">{ "+" }</strong>
                <input type="number" oninput={on_num2} style="width: 40%; padding: 5px;" />
            </div>
            <button onclick={on_calc} style="width: 100%; padding: 8px; background: #2e7d32; color: white; border: none; border-radius: 5px; cursor: pointer;">{ "Hesapla" }</button>
            if let Some(r) = *result {
                <div style="margin-top: 10px; padding: 5px; background: #dcedc8; color: #33691e; border-radius: 5px; text-align: center; font-weight: bold;">{ format!("Sonuç: {}", r) }</div>
            }
        </div>
    }
}

#[function_component(DynamicWindow)]
fn dynamic_window(props: &WindowProps) -> Html {
    let schema = match &props.app.mode {
        AppMode::Dynamic(s) => s.clone(),
        _ => return html! { <div>{ "Hata" }</div> },
    };

    let result = use_state(|| None::<f64>);
    let instance = props.app.instance.clone();

    html! {
        <div style="padding: 15px; background: #e0f7fa;">
            <div style="display: flex; flex-direction: column; gap: 10px;">
                {
                    for schema.bilesenler.iter().map(|comp| {
                        if comp.tip == "sayi_girisi" {
                            html! {
                                <div>
                                    <label style="font-size: 12px; color: #555;">{ &comp.etiket }</label>
                                    <input type="number" id={comp.id.clone()} style="width: 100%; padding: 5px; box-sizing: border-box;" />
                                </div>
                            }
                        } else if comp.tip == "buton" {
                            let func_name = comp.fonksiyon.clone().unwrap_or_default();
                            let inputs = comp.girdiler.clone().unwrap_or_default();
                            let inst = instance.clone();
                            let res = result.clone();
                            
                            let on_click = Callback::from(move |_| {
                                let window = web_sys::window().unwrap();
                                let document = window.document().unwrap();
                                
                                let mut args = js_sys::Array::new();
                                for input_id in &inputs {
                                    let val = if let Some(el) = document.get_element_by_id(input_id) {
                                        el.dyn_into::<HtmlInputElement>().unwrap().value().parse::<i32>().unwrap_or(0)
                                    } else { 0 };
                                    args.push(&JsValue::from(val));
                                }

                                let exports = Reflect::get(&inst, &"exports".into()).unwrap();
                                if let Ok(func_val) = Reflect::get(&exports, &JsValue::from_str(&func_name)) {
                                    let func: Function = func_val.into();
                                    if let Ok(r) = Reflect::apply(&func, &JsValue::NULL, &args) {
                                        res.set(Some(r.as_f64().unwrap_or(0.0)));
                                    }
                                }
                            });

                            html! {
                                <button onclick={on_click} id={comp.id.clone()} style="width: 100%; padding: 10px; background: #00838f; color: white; border: none; border-radius: 5px; cursor: pointer; font-weight: bold;">
                                    { &comp.etiket }
                                </button>
                            }
                        } else {
                            html! { <div style="color: red;">{ "Bilinmeyen tip!" }</div> }
                        }
                    })
                }
            </div>

            if let Some(r) = *result {
                <div style="margin-top: 15px; padding: 10px; background: #b2ebf2; color: #006064; border-radius: 5px; text-align: center; font-weight: bold; font-size: 18px;">
                    { format!("Sonuç: {}", r) }
                </div>
            }
        </div>
    }
}

// =========================================================================
// 5. ANA UYGULAMA (İŞLETİM SİSTEMİ MASAÜSTÜ)
// =========================================================================
#[function_component(App)]
fn app() -> Html {
    let status = use_state(|| "Orbit OS Başlatıldı.".to_string());
    let windows = use_state(|| Vec::<AppData>::new());
    let window_counter = use_mut_ref(|| 1usize);

    let default_code = r##"static UI_SCHEMA: &str = r#"{
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
"##.to_string();

    let input_code = use_state(|| default_code);
    let github_url = use_state(|| "".to_string());

    // YENİ: Pencere Kapatma Fonksiyonu
    let close_window = {
        let windows = windows.clone();
        Callback::from(move |id_to_close: usize| {
            let mut current_windows = (*windows).clone();
            current_windows.retain(|app| app.id != id_to_close); // X'e basılan pencereyi listeden siler
            windows.set(current_windows);
        })
    };

    let analyze_and_load_wasm = {
        let status = status.clone();
        let windows = windows.clone();
        let window_counter = window_counter.clone();

        Callback::from(move |bytes: Vec<u8>| {
            let status = status.clone();
            let windows = windows.clone();
            let mut current_windows = (*windows).clone();
            let window_counter_async = window_counter.clone();
            let current_id = *window_counter_async.borrow();

            spawn_local(async move {
                let promise = WebAssembly::instantiate_buffer(&bytes, &Object::new());
                if let Ok(wasm_result) = JsFuture::from(promise).await {
                    let instance_val = Reflect::get(&wasm_result, &"instance".into()).unwrap();
                    let instance: WebAssembly::Instance = instance_val.into();
                    let exports = Reflect::get(&instance, &"exports".into()).unwrap();
                    
                    let has_schema = Reflect::has(&exports, &"get_schema_ptr".into()).unwrap_or(false);
                    
                    if has_schema {
                        let ptr_func: Function = Reflect::get(&exports, &"get_schema_ptr".into()).unwrap().into();
                        let len_func: Function = Reflect::get(&exports, &"get_schema_len".into()).unwrap().into();
                        
                        let ptr = ptr_func.call0(&JsValue::NULL).unwrap().as_f64().unwrap() as u32;
                        let len = len_func.call0(&JsValue::NULL).unwrap().as_f64().unwrap() as u32;

                        let memory = Reflect::get(&exports, &"memory".into()).unwrap();
                        let memory_obj: WebAssembly::Memory = memory.into();
                        let memory_buffer = memory_obj.buffer();
                        
                        let uint8_array = js_sys::Uint8Array::new_with_byte_offset_and_length(&memory_buffer, ptr, len);
                        let mut string_bytes = vec![0; len as usize];
                        uint8_array.copy_to(&mut string_bytes);

                        if let Ok(json_str) = String::from_utf8(string_bytes) {
                            if let Ok(schema) = serde_json::from_str::<UISchema>(&json_str) {
                                current_windows.push(AppData { id: current_id, mode: AppMode::Dynamic(schema), instance });
                                status.set(format!("✅ Dinamik Eklenti Çizildi! (ID: {})", current_id));
                            } else {
                                status.set("❌ Şema formatı hatalı!".to_string());
                            }
                        }
                    } else {
                        let has_graphics = Reflect::has(&exports, &"generate_frame".into()).unwrap_or(false);
                        let has_math = Reflect::has(&exports, &"topla".into()).unwrap_or(false);
                        let mode = if has_graphics { AppMode::Graphics } else if has_math { AppMode::Calculator } else { AppMode::UnknownTerminal };
                        
                        current_windows.push(AppData { id: current_id, mode, instance });
                        status.set(format!("✅ Klasik eklenti eklendi (ID: {}).", current_id));
                    }
                    
                    windows.set(current_windows);
                    *window_counter_async.borrow_mut() += 1; 
                } else {
                    status.set("❌ WASM çalıştırılamadı!".to_string());
                }
            });
        })
    };

    let on_code_change = { let ic = input_code.clone(); Callback::from(move |e: Event| { let input: HtmlTextAreaElement = e.target_unchecked_into(); ic.set(input.value()); }) };
    let on_github_change = { let gu = github_url.clone(); Callback::from(move |e: Event| { let input: HtmlInputElement = e.target_unchecked_into(); gu.set(input.value()); }) };

    let on_compile = {
        let status = status.clone(); let input_code = input_code.clone(); let analyze = analyze_and_load_wasm.clone();
        Callback::from(move |_| {
            let status = status.clone(); let code = (*input_code).clone(); let analyze = analyze.clone();
            spawn_local(async move {
                status.set("☁️ Bulutta (Docker) derleniyor...".to_string());
                match Request::post("http://127.0.0.1:3000/compile").header("Content-Type", "text/plain").body(code).unwrap().send().await {
                    Ok(response) => { if response.ok() { analyze.emit(response.binary().await.unwrap()); } else { status.set(format!("❌ Hata: {}", response.text().await.unwrap())); } },
                    Err(_) => { status.set("❌ Sunucuya ulaşılamadı. Orbit Backend açık mı?".to_string()); }
                }
            });
        })
    };

    let on_github_compile = {
        let status = status.clone(); let github_url = github_url.clone(); let analyze = analyze_and_load_wasm.clone();
        Callback::from(move |_| {
            let status = status.clone(); let url = (*github_url).clone(); let analyze = analyze.clone();
            if url.trim().is_empty() { return; }
            spawn_local(async move {
                status.set("🐙 GitHub'dan çekiliyor ve Docker'da derleniyor...".to_string());
                match Request::post("http://127.0.0.1:3000/github").header("Content-Type", "text/plain").body(url).unwrap().send().await {
                    Ok(response) => { if response.ok() { analyze.emit(response.binary().await.unwrap()); } else { status.set(format!("❌ Hata: {}", response.text().await.unwrap())); } },
                    Err(_) => { status.set("❌ Sunucuya ulaşılamadı. Orbit Backend açık mı?".to_string()); }
                }
            });
        })
    };

    html! {
        <div style="padding: 20px; font-family: system-ui, sans-serif; min-height: 100vh; background: #f0f2f5;">
            <div style="max-width: 1200px; margin: auto; display: flex; gap: 30px;">
                <div style="flex: 1; min-width: 320px; max-width: 400px; z-index: 1;">
                    <h1 style="color: #1a237e;">{ "Orbit OS ☁️" }</h1>
                    
                    <div style="background: white; padding: 20px; border-radius: 12px; box-shadow: 0 4px 6px rgba(0,0,0,0.05); margin-bottom: 20px;">
                        <h4 style="margin-top: 0; color: #333;">{ "🐙 GitHub'dan Yükle" }</h4>
                        <input type="text" value={(*github_url).clone()} onchange={on_github_change} placeholder="https://github.com/..." style="width: 100%; padding: 10px; margin-bottom: 10px; border: 1px solid #ddd; border-radius: 6px; box-sizing: border-box;" />
                        <button onclick={on_github_compile} style="width: 100%; padding: 12px; background: #24292e; color: white; border: none; border-radius: 6px; cursor: pointer; font-weight: bold;">{ "Çek ve Çalıştır" }</button>
                    </div>

                    <div style="background: #282c34; padding: 20px; border-radius: 12px; box-shadow: inset 0 2px 10px rgba(0,0,0,0.5);">
                        <h4 style="color: #abb2bf; margin-top: 0;">{ "💻 Kodu Elle Yaz" }</h4>
                        <textarea value={(*input_code).clone()} onchange={on_code_change} style="width: 100%; height: 280px; background: #1e1e1e; color: #98c379; border: none; font-family: monospace; font-size: 13px; resize: vertical;" spellcheck="false" />
                        <button onclick={on_compile} style="margin-top: 15px; width: 100%; padding: 12px; background: #0277bd; color: white; border: none; border-radius: 6px; cursor: pointer; font-weight: bold;">{ "Bulutta Derle" }</button>
                    </div>

                    <div style="padding: 15px; background: #e3f2fd; border-radius: 12px; color: #0d47a1; margin-top: 20px; border-left: 5px solid #0d47a1;">
                        <strong>{ "Durum: " }</strong> <br/> { (*status).clone() }
                    </div>
                </div>

                // YENİ: MASAÜSTÜ ALANI (position: relative ve overflow: hidden eklendi)
                <div style="flex: 2; background: #e0e5ec; border-radius: 12px; box-shadow: inset 0 4px 20px rgba(0,0,0,0.08); min-height: 700px; position: relative; overflow: hidden;">
                    
                    {
                        for windows.iter().map(|app| {
                            // Başlık dinamik olarak oluşturuluyor
                            let title = match &app.mode {
                                AppMode::Graphics => format!("🎨 Grafik Motoru (ID: {})", app.id),
                                AppMode::Calculator => format!("🧮 Hesap Makinesi (ID: {})", app.id),
                                AppMode::UnknownTerminal => format!("🖥️ Terminal (ID: {})", app.id),
                                AppMode::Dynamic(s) => format!("✨ {} (ID: {})", s.isim, app.id),
                            };

                            // Her uygulama ortak Sürüklenebilir WindowFrame içine alınıyor
                            html! {
                                <WindowFrame id={app.id} title={title} on_close={close_window.clone()}>
                                    {
                                        match &app.mode {
                                            AppMode::Graphics => html! { <GraphicsWindow app={app.clone()} /> },
                                            AppMode::Calculator => html! { <CalculatorWindow app={app.clone()} /> },
                                            AppMode::UnknownTerminal => html! { 
                                                <div style="padding: 15px; background: #000; color: #0f0; font-family: monospace;">
                                                    { "> Bilinmeyen eklenti..." }
                                                </div> 
                                            },
                                            AppMode::Dynamic(_) => html! { <DynamicWindow app={app.clone()} /> },
                                        }
                                    }
                                </WindowFrame>
                            }
                        })
                    }
                    
                    if windows.is_empty() {
                        <div style="position: absolute; top: 50%; left: 50%; transform: translate(-50%, -50%); text-align: center; color: #9aa;">
                            <svg width="64" height="64" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><rect x="2" y="3" width="20" height="14" rx="2" ry="2"></rect><line x1="8" y1="21" x2="16" y2="21"></line><line x1="12" y1="17" x2="12" y2="21"></line></svg>
                            <p style="font-weight: bold;">{ "Masaüstü Temiz" }</p>
                            <p style="font-size: 14px;">{ "Soldan bir uygulama derleyin veya GitHub linki yapıştırın." }</p>
                        </div>
                    }
                </div>
            </div>
        </div>
    }
}

fn main() {
    yew::Renderer::<App>::new().render();
}
