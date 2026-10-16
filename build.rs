use std::fs::File;
use std::io::Write;
use std::path::Path;

fn generate_ico(path: &Path) {
    let width = 32u32;
    let height = 32u32;
    let bpp = 32u32;

    let color_data_size = width * height * 4;
    let mask_row_bytes = width.div_ceil(32) * 4; // 4 bytes for 32 pixels
    let mask_data_size = mask_row_bytes * height;
    let bmp_header_size = 40u32;
    let image_bytes = bmp_header_size + color_data_size + mask_data_size;

    let mut buf = Vec::with_capacity((22 + image_bytes) as usize);

    // ICONDIR
    buf.extend_from_slice(&0u16.to_le_bytes()); // idReserved
    buf.extend_from_slice(&1u16.to_le_bytes()); // idType (1 = icon)
    buf.extend_from_slice(&1u16.to_le_bytes()); // idCount (1 image)

    // ICONDIRENTRY
    buf.push(width as u8); // bWidth
    buf.push(height as u8); // bHeight
    buf.push(0); // bColorCount
    buf.push(0); // bReserved
    buf.extend_from_slice(&1u16.to_le_bytes()); // wPlanes
    buf.extend_from_slice(&(bpp as u16).to_le_bytes()); // wBitCount
    buf.extend_from_slice(&image_bytes.to_le_bytes()); // dwBytesInRes
    buf.extend_from_slice(&22u32.to_le_bytes()); // dwImageOffset (6 + 16)

    // BITMAPINFOHEADER
    buf.extend_from_slice(&40u32.to_le_bytes()); // biSize
    buf.extend_from_slice(&(width as i32).to_le_bytes()); // biWidth
    buf.extend_from_slice(&((height * 2) as i32).to_le_bytes()); // biHeight (double for icon)
    buf.extend_from_slice(&1u16.to_le_bytes()); // biPlanes
    buf.extend_from_slice(&(bpp as u16).to_le_bytes()); // biBitCount
    buf.extend_from_slice(&0u32.to_le_bytes()); // biCompression (BI_RGB)
    buf.extend_from_slice(&(color_data_size + mask_data_size).to_le_bytes()); // biSizeImage
    buf.extend_from_slice(&0i32.to_le_bytes()); // biXPelsPerMeter
    buf.extend_from_slice(&0i32.to_le_bytes()); // biYPelsPerMeter
    buf.extend_from_slice(&0u32.to_le_bytes()); // biClrUsed
    buf.extend_from_slice(&0u32.to_le_bytes()); // biClrImportant

    // Pixel data: bottom-to-top, left-to-right (32x32)
    // Draw an elegant power button icon: dark slate circle with bright cyan power symbol
    let center_x = 15.5f32;
    let center_y = 15.5f32;
    let radius = 13.5f32;

    for y in 0..height {
        // y is from bottom (0) to top (31)
        for x in 0..width {
            let dx = x as f32 - center_x;
            let dy = y as f32 - center_y;
            let dist = (dx * dx + dy * dy).sqrt();

            if dist <= radius {
                // In bottom-to-top coords, top is +y (dy > 0)
                let is_top_gap = dy > 0.0 && dx.abs() < 3.0;
                let is_arc = (6.0..=8.5).contains(&dist) && !is_top_gap;
                let is_stem = dx.abs() <= 1.2 && (2.0..=10.0).contains(&dy);

                if is_arc || is_stem {
                    // Bright Cyan #00F0FF (B, G, R, A)
                    buf.push(0xFF); // B
                    buf.push(0xF0); // G
                    buf.push(0x00); // R
                    buf.push(0xFF); // A
                } else {
                    // Dark Slate Blue #1E293B (B=0x3B, G=0x29, R=0x1E, A=0xFF)
                    buf.push(0x3B);
                    buf.push(0x29);
                    buf.push(0x1E);
                    buf.push(0xFF);
                }
            } else {
                // Transparent pixel
                buf.push(0);
                buf.push(0);
                buf.push(0);
                buf.push(0);
            }
        }
    }

    // 1bpp AND mask (32 rows * 4 bytes = 128 bytes)
    // 0 means opaque/use alpha channel
    buf.resize(buf.len() + mask_data_size as usize, 0);

    let mut file = File::create(path).expect("Failed to create app.ico");
    file.write_all(&buf).expect("Failed to write app.ico");
}

fn main() {
    let ico_path = Path::new("app.ico");
    if !ico_path.exists() {
        generate_ico(ico_path);
    }

    let uac_level = "asInvoker";

    let manifest_content = format!(
        r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<assembly xmlns="urn:schemas-microsoft-com:asm.v1" manifestVersion="1.0">
  <assemblyIdentity version="1.0.0.0" processorArchitecture="*" name="IdleOff" type="win32"/>
  <description>IdleOff Auto Shutdown Utility</description>
  <trustInfo xmlns="urn:schemas-microsoft-com:asm.v3">
    <security>
      <requestedPrivileges>
        <requestedExecutionLevel level="{uac_level}" uiAccess="false"/>
      </requestedPrivileges>
    </security>
  </trustInfo>
  <compatibility xmlns="urn:schemas-microsoft-com:compatibility.v1">
    <application>
      <supportedOS Id="{{8e0f7a12-bfb3-4fe8-b9a5-48fd50a15a9a}}"/>
    </application>
  </compatibility>
</assembly>
"#
    );

    let manifest_path = Path::new("app.manifest");
    let mut file = File::create(manifest_path).expect("Failed to create app.manifest");
    file.write_all(manifest_content.as_bytes())
        .expect("Failed to write app.manifest");

    let rc_path = Path::new("app.rc");
    let mut file = File::create(rc_path).expect("Failed to create app.rc");
    file.write_all(b"1 ICON \"app.ico\"\n1 24 \"app.manifest\"\n")
        .expect("Failed to write app.rc");

    #[cfg(windows)]
    {
        let _ = embed_resource::compile("app.rc", embed_resource::NONE);
    }
}
