// SPDX-License-Identifier: GPL-3.0-or-later
//! M5 exit criterion 5 on the UI paths: `SetBackground` and `Graphic` paths cannot reach
//! files outside the `Plugins/` tree (traversal, absolute paths, drive letters, symlinks,
//! over-long paths), refused formats and hostile image headers fail cleanly at call time
//! without decoding, and plugin-controlled sizes, numbers, texts and object counts are
//! bounded. Every attempt fails cleanly: the host keeps running.
//!
//! Spec: `lua-plugin-api.md` §4.4, §13.8; decision 0015 §4.3–§4.4, §7.

#[path = "fixtures/harness.rs"]
mod harness;

use harness::Fx;

fn fixture() -> Fx {
    let mut fx = Fx::new();
    let outside = fx.dir.path().join("outside");
    std::fs::create_dir_all(&outside).unwrap();
    std::fs::write(outside.join("secret.tga"), harness::tga(2, 2)).unwrap();
    std::fs::write(outside.join("secret.jpg"), harness::jpeg(2, 2)).unwrap();
    fx.file("Evil/Art/ok.tga", harness::tga(4, 4));
    fx.file("Evil/Art/ok.jpg", harness::jpeg(4, 4));
    fx.file("Evil/Art/pic.png", b"\x89PNG\r\n\x1a\n0000000000000000");
    fx.file("Evil/Art/pic.bmp", b"BM0000000000000000000000");
    fx.file("Evil/Art/junk.tga", b"\x00\x00\xFF\xFF");
    fx.file("Evil/Art/empty.tga", b"");
    fx.file("Evil/Art/notjpeg.jpg", b"GIF89a00000000");
    // TGA and JPEG headers that declare enormous images.
    let mut huge_tga = harness::tga(1, 1);
    huge_tga[12..14].copy_from_slice(&u16::MAX.to_le_bytes());
    huge_tga[14..16].copy_from_slice(&u16::MAX.to_le_bytes());
    fx.file("Evil/Art/huge.tga", huge_tga);
    fx.file("Evil/Art/huge.jpg", harness::jpeg(u16::MAX, u16::MAX));
    std::fs::create_dir_all(fx.plugins.join("Evil/Art/folder.tga")).unwrap();
    #[cfg(unix)]
    {
        std::os::unix::fs::symlink(
            outside.join("secret.tga"),
            fx.plugins.join("Evil/Art/link.tga"),
        )
        .unwrap();
        std::os::unix::fs::symlink(&outside, fx.plugins.join("Evil/Out")).unwrap();
    }
    fx.plugin("Evil", "Probe", Some("Probe"), "c = Turbine.UI.Control()");
    fx.refresh();
    fx.load("Probe");
    fx
}

fn set(fx: &Fx, path: &str) -> Result<(), String> {
    let p = path.replace('\\', "\\\\").replace('\'', "\\'");
    fx.try_eval::<()>("Probe", &format!("c:SetBackground('{p}')"))
        .map_err(|e| e.to_string())
}

#[test]
fn paths_inside_the_tree_work() {
    let fx = fixture();
    for p in ["Evil/Art/ok.tga", "EVIL/art/OK.TGA", "Evil\\Art\\ok.jpg"] {
        assert!(set(&fx, p).is_ok(), "{p}: {:?}", set(&fx, p));
    }
}

#[test]
fn escapes_are_refused() {
    let fx = fixture();
    let outside = fx.dir.path().join("outside/secret.tga");
    let cases = [
        "../outside/secret.tga".to_owned(),
        "Evil/../../outside/secret.tga".to_owned(),
        "Evil/Art/../../../outside/secret.tga".to_owned(),
        "./Evil/Art/ok.tga".to_owned(),
        "Evil//Art/ok.tga".to_owned(),
        outside.display().to_string(),
        "/etc/passwd".to_owned(),
        "C:\\Windows\\win.ini".to_owned(),
        "c:/outside/secret.tga".to_owned(),
        "\\\\server\\share\\secret.tga".to_owned(),
        "Evil/Art/link.tga".to_owned(),
        "Evil/Out/secret.tga".to_owned(),
        "Evil/Art/folder.tga".to_owned(),
        "Evil/Art/missing.tga".to_owned(),
        format!("Evil/{}.tga", "a".repeat(5000)),
        format!("{}ok.tga", "Evil/Art/".repeat(400)),
    ];
    for p in &cases {
        let r = set(&fx, p);
        assert!(r.is_err(), "{p} must be refused");
        let msg = r.unwrap_err();
        assert!(
            msg.len() < 1024,
            "the error message is bounded: {} bytes",
            msg.len()
        );
    }
    // A NUL byte inside the path.
    let r: Result<(), _> = fx.try_eval("Probe", "c:SetBackground('Evil/Art/ok.tga\\0.png')");
    assert!(r.is_err());
    // The same through a Graphic.
    let r: Result<(), _> = fx.try_eval(
        "Probe",
        "c:SetBackground(Turbine.UI.Graphic('../outside/secret.tga'))",
    );
    assert!(r.is_err());
    // A refused call leaves the previous background.
    set(&fx, "Evil/Art/ok.tga").unwrap();
    let _ = set(&fx, "../outside/secret.tga");
    let bg: String = fx.eval("Probe", "return c:GetBackground()");
    assert_eq!(bg, "Evil/Art/ok.tga");
    fx.assert_no_errors();
}

#[test]
fn refused_formats_and_hostile_headers() {
    let fx = fixture();
    for p in [
        "Evil/Art/pic.png",
        "Evil/Art/pic.bmp",
        "Evil/Art/junk.tga",
        "Evil/Art/empty.tga",
        "Evil/Art/notjpeg.jpg",
    ] {
        assert!(
            set(&fx, p).is_err(),
            "{p} must raise the unsupported-format error"
        );
    }
    // Enormous declared sizes: refused by the header probe, or accepted without any
    // decode at call time; either way the apartment's memory stays small.
    for p in ["Evil/Art/huge.tga", "Evil/Art/huge.jpg"] {
        let _ = set(&fx, p);
        let (used, _) = fx.manager.apartment_memory("Probe").unwrap();
        assert!(used < 64 << 20, "{p}: {used} bytes in use");
    }
}

#[test]
fn hostile_numbers_and_sizes() {
    let fx = fixture();
    let _: () = fx.eval(
        "Probe",
        "for _, v in ipairs({ -1, 0/0, 1/0, -1/0, 2^53, -2^63, 4294967296, 0x41FFFFFF }) do \
           c:SetBackground(v) c:SetSize(v, v) c:SetPosition(v, v) c:SetOpacity(v) c:SetStretchMode(v) \
           c:SetZOrder(v) c:SetMinimumSize(v, v) c:AttachEdges(v, v, v, v) \
         end \
         c:SetMinimumSize(0, 0) c:SetMaximumSize(0, 0) c:AttachEdges(0, 0, 0, 0) \
         local w = Turbine.UI.Window() w:SetVisible(true) \
         for _, v in ipairs({ 0/0, 1/0, -1e300 }) do w:SetRotation({ x = v, y = v, z = v }) end \
         local sb = Turbine.UI.ScrollBar() \
         for _, v in ipairs({ 0/0, 1/0, -1/0, 2^64, -2^64 }) do sb:SetMinimum(v) sb:SetMaximum(v) sb:SetValue(v) \
           sb:SetLargeChange(v) sb:SetSmallChange(v) end",
    );
    // An enormous control is a warning to the apartment, not an error or a clamp.
    let w: f64 = fx.eval(
        "Probe",
        "local d = Turbine.UI.Control() d:SetSize(100000, 10) return d:GetWidth()",
    );
    assert_eq!(w, 100_000.0);
}

#[test]
fn text_is_bounded() {
    let fx = fixture();
    let n: f64 = fx.eval(
        "Probe",
        "local l = Turbine.UI.Label() l:SetText(string.rep('x', 1000000)) return l:GetTextLength()",
    );
    assert_eq!(
        n,
        f64::from(1u32 << 17),
        "one text keeps at most MAX_TEXT_UNITS units"
    );
    // Many controls holding long text: refused past the per-apartment bound.
    let r: (bool, f64) = fx.eval(
        "Probe",
        "local s = string.rep('y', 131072) local keep = {} local ok, n = true, 0 \
         while ok and n < 2000 do local l = Turbine.UI.Label() ok = pcall(l.SetText, l, s) n = n + 1 keep[n] = l end \
         return ok, n",
    );
    assert!(!r.0, "refused past the per-apartment text bound");
    let limit = tessera_plugins::ui::MAX_TEXT_BYTES_PER_APARTMENT as f64 / 131_072.0;
    assert!(r.1 <= limit + 2.0, "{} labels", r.1);
    let d: f64 = fx.eval(
        "Probe",
        "local s = Turbine.UI.Lotro.Shortcut(4, string.rep('z', 100000)) return #s:GetData()",
    );
    assert!(d <= 4096.0);
}

#[test]
fn unload_destroys_every_element() {
    let mut fx = fixture();
    let _: () = fx.eval(
        "Probe",
        "keep = {} for i = 1, 50 do local w = Turbine.UI.Lotro.Window() w:SetVisible(true) keep[i] = w \
           local l = Turbine.UI.Label() l:SetParent(w) end",
    );
    let before = fx.pui.ui().borrow().element_count();
    assert!(before > 50 * 2);
    fx.manager.unload_apartment("Probe").unwrap();
    fx.deliver();
    let after = fx.pui.ui().borrow().element_count();
    assert!(
        after < 10,
        "{before} elements before, {after} after the unload"
    );
    assert_eq!(fx.pui.object_count("Probe"), 0);
}

/// Random calls of every UI method with hostile arguments, mixed with synthetic input,
/// never panic the host (a deterministic, seeded stand-in for a fuzz target).
#[test]
fn random_calls_never_panic() {
    let mut fx = fixture();
    let _: () = fx.eval(
        "Probe",
        "objs = {} \
         local classes = { Turbine.UI.Control, Turbine.UI.Window, Turbine.UI.Label, Turbine.UI.TextBox, \
           Turbine.UI.Button, Turbine.UI.CheckBox, Turbine.UI.ScrollBar, Turbine.UI.ListBox, Turbine.UI.TreeView, \
           Turbine.UI.TreeNode, Turbine.UI.Lotro.Window, Turbine.UI.Lotro.GoldWindow, Turbine.UI.Lotro.ScrollBar, \
           Turbine.UI.Lotro.Quickslot, Turbine.UI.ContextMenu, Turbine.UI.MenuItem, Turbine.UI.Lotro.Shortcut, \
           Turbine.UI.Graphic, Turbine.UI.DragDropInfo } \
         for i = 1, 60 do local c = classes[(i % #classes) + 1] local ok, o = pcall(c) if ok then objs[#objs+1] = o end end \
         methods = {} \
         for _, c in ipairs(classes) do local t = c while type(t) == 'table' do \
           for k, v in pairs(t) do if type(v) == 'function' and k ~= 'Constructor' and k ~= 'IsA' then methods[#methods+1] = k end end \
           local mt = getmetatable(t) t = mt and mt.__index end end \
         args = { n = 0 }",
    );
    let counts: (f64, f64) = fx.eval("Probe", "return #objs, #methods");
    assert!(counts.0 >= 50.0 && counts.1 >= 150.0, "{counts:?}");
    let mut seed: u64 = 0x9E37_79B9_7F4A_7C15;
    let mut next = move || {
        seed ^= seed << 13;
        seed ^= seed >> 7;
        seed ^= seed << 17;
        seed
    };
    let values = [
        "nil",
        "0",
        "-1",
        "1",
        "0/0",
        "1/0",
        "-1/0",
        "2^53",
        "-2^63",
        "4294967296",
        "0x41000001",
        "'x'",
        "''",
        "'Evil/Art/ok.tga'",
        "'../outside/secret.tga'",
        "string.rep('w', 5000)",
        "{}",
        "{ x = 1/0, y = 0/0, z = 'q' }",
        "true",
        "false",
        "objs[1]",
        "objs[2]",
        "objs[7]",
        "objs[9]",
        "objs[10]",
        "objs[15]",
        "objs[16]",
        "function(a, b) return a end",
        "Turbine.UI.Color(2, -1, 0/0, 1/0)",
        "Turbine.UI.Color.Red",
    ];
    for round in 0..200 {
        let mut code = String::new();
        for _ in 0..100 {
            let obj = next() % 60 + 1;
            let method = next() % 400;
            let a = values[(next() % values.len() as u64) as usize];
            let b = values[(next() % values.len() as u64) as usize];
            let c = values[(next() % values.len() as u64) as usize];
            code.push_str(&format!(
                "do local o = objs[{obj}] local m = methods[({method} % #methods) + 1] \
                 if o and type(o[m]) == 'function' then pcall(o[m], o, {a}, {b}, {c}) end end\n"
            ));
        }
        let r: Result<(), _> = fx.try_eval("Probe", &code);
        assert!(r.is_ok(), "round {round}: {r:?}");
        let x = (next() % 1600) as i32;
        let y = (next() % 900) as i32;
        match next() % 4 {
            0 => fx.click(x, y, 1),
            1 => fx.click(x, y, 2),
            2 => fx.wheel(x, y, next() % 2 == 0),
            _ => {
                fx.key("ToggleBags", 0x1000_0094);
            }
        }
        fx.frame(0.016);
    }
    let alive: f64 = fx.eval("Probe", "return 6 * 7");
    assert_eq!(alive, 42.0);
    fx.manager.unload_all();
    fx.deliver();
}
