#![deny(clippy::all)]

use napi::bindgen_prelude::*;
use napi::Result;
use napi_derive::napi;

#[napi]
pub fn rgba_to_yuv420p(rgba: &[u8], width: u32, height: u32) -> Result<Buffer> {
  let width = width as usize;
  let height = height as usize;
  let rgba_data = rgba; // 直接获取 Buffer 内部数据

  // 验证输入长度
  let expected_rgba_size = width * height * 4;
  if rgba_data.len() != expected_rgba_size {
    return Err(Error::from_reason(format!(
      "RGBA 缓冲区大小不匹配: 预期 {} 字节，实际 {}",
      expected_rgba_size,
      rgba_data.len()
    )));
  }

  // 计算 YUV 各平面大小
  let y_size = width * height;
  let uv_size = y_size / 4;
  let total_size = y_size + uv_size * 2;

  // 初始化输出缓冲区
  let mut yuv = vec![0u8; total_size];
  let (y_plane, rest) = yuv.split_at_mut(y_size);
  let (u_plane, v_plane) = rest.split_at_mut(uv_size);

  // 转换为 Y 分量（逐像素）
  for i in 0..y_size {
    let idx = i * 4;
    let r = rgba_data[idx] as f32;
    let g = rgba_data[idx + 1] as f32;
    let b = rgba_data[idx + 2] as f32;

    // BT.601 标准 Y 转换公式
    let y: f32 = 0.257 * r + 0.504 * g + 0.098 * b + 16.0;
    y_plane[i] = y.clamp(16.0, 235.0) as u8;
  }

  // 转换为 UV 分量（2x2 像素块平均）
  let uv_width = width / 2;
  let uv_height = height / 2;

  for uv_y in 0..uv_height {
    let y_row1 = uv_y * 2;
    let y_row2 = y_row1 + 1;
    let uv_row_start = uv_y * uv_width;

    for uv_x in 0..uv_width {
      let x1 = uv_x * 2;
      let x2 = x1 + 1;

      // 2x2 块中四个像素的索引
      let idx1 = y_row1 * width + x1;
      let idx2 = y_row1 * width + x2;
      let idx3 = y_row2 * width + x1;
      let idx4 = y_row2 * width + x2;

      // 提取四个像素的 RGB 值
      let r1 = rgba_data[idx1 * 4] as f32;
      let g1 = rgba_data[idx1 * 4 + 1] as f32;
      let b1 = rgba_data[idx1 * 4 + 2] as f32;

      let r2 = rgba_data[idx2 * 4] as f32;
      let g2 = rgba_data[idx2 * 4 + 1] as f32;
      let b2 = rgba_data[idx2 * 4 + 2] as f32;

      let r3 = rgba_data[idx3 * 4] as f32;
      let g3 = rgba_data[idx3 * 4 + 1] as f32;
      let b3 = rgba_data[idx3 * 4 + 2] as f32;

      let r4 = rgba_data[idx4 * 4] as f32;
      let g4 = rgba_data[idx4 * 4 + 1] as f32;
      let b4 = rgba_data[idx4 * 4 + 2] as f32;

      // 计算 RGB 平均值
      let r_avg = (r1 + r2 + r3 + r4) / 4.0;
      let g_avg = (g1 + g2 + g3 + g4) / 4.0;
      let b_avg = (b1 + b2 + b3 + b4) / 4.0;

      // BT.601 标准 UV 转换公式
      let u: f32 = (-0.148 * r_avg - 0.291 * g_avg + 0.439 * b_avg) + 128.0;
      let v: f32 = (0.439 * r_avg - 0.368 * g_avg - 0.071 * b_avg) + 128.0;

      // 写入 UV 平面（裁剪范围）
      let uv_idx = uv_row_start + uv_x;
      u_plane[uv_idx] = u.clamp(16.0, 240.0) as u8;
      v_plane[uv_idx] = v.clamp(16.0, 240.0) as u8;
    }
  }
  Ok(yuv.into())
}

#[napi]
pub fn yuv420p_to_rgba(yuv: &[u8], width: u32, height: u32) -> Result<Buffer> {
  let width = width as usize;
  let height = height as usize;
  let yuv_data = yuv;

  // 验证输入长度
  let y_size = width * height;
  let uv_size = y_size / 4;
  let expected_yuv_size = y_size + uv_size * 2;
  if yuv_data.len() != expected_yuv_size {
    return Err(Error::from_reason(format!(
      "YUV 缓冲区大小不匹配: 预期 {} 字节，实际 {}",
      expected_yuv_size,
      yuv_data.len()
    )));
  }

  // 分割 YUV 平面
  let (y_plane, rest) = yuv_data.split_at(y_size);
  let (u_plane, v_plane) = rest.split_at(uv_size);

  // 初始化 RGBA 输出缓冲区（Alpha 通道默认 255）
  let mut rgba = vec![0u8; width * height * 4];

  // 转换参数（BT.601 标准）
  const C: f32 = 1.164;
  let uv_width = width / 2;
  let half_height = height / 2;

  // 逐 2x2 块转换
  for uv_y in 0..half_height {
    let y_row_start1 = uv_y * 2 * width;
    let y_row_start2 = (uv_y * 2 + 1) * width;
    let uv_row_start = uv_y * uv_width;

    for uv_x in 0..uv_width {
      // 获取当前块的 UV 分量
      let u = u_plane[uv_row_start + uv_x] as f32 - 128.0;
      let v = v_plane[uv_row_start + uv_x] as f32 - 128.0;

      // 预计算共享系数
      let d: f32 = 1.596 * v;
      let e: f32 = 0.392 * u;
      let f: f32 = 0.813 * v;
      let g: f32 = 2.017 * u;

      // 像素 1: (2*uv_x, 2*uv_y)
      let y1 = y_plane[y_row_start1 + 2 * uv_x] as f32 - 16.0;
      let r = (C * y1 + d).clamp(0.0, 255.0) as u8;
      let g_val = (C * y1 - e - f).clamp(0.0, 255.0) as u8;
      let b = (C * y1 + g).clamp(0.0, 255.0) as u8;
      let idx1 = (y_row_start1 + 2 * uv_x) * 4;
      rgba[idx1] = r;
      rgba[idx1 + 1] = g_val;
      rgba[idx1 + 2] = b;
      rgba[idx1 + 3] = 255; // Alpha

      // 像素 2: (2*uv_x+1, 2*uv_y)
      let y2 = y_plane[y_row_start1 + 2 * uv_x + 1] as f32 - 16.0;
      let r = (C * y2 + d).clamp(0.0, 255.0) as u8;
      let g_val = (C * y2 - e - f).clamp(0.0, 255.0) as u8;
      let b = (C * y2 + g).clamp(0.0, 255.0) as u8;
      let idx2 = (y_row_start1 + 2 * uv_x + 1) * 4;
      rgba[idx2] = r;
      rgba[idx2 + 1] = g_val;
      rgba[idx2 + 2] = b;
      rgba[idx2 + 3] = 255;

      // 像素 3: (2*uv_x, 2*uv_y+1)
      let y3 = y_plane[y_row_start2 + 2 * uv_x] as f32 - 16.0;
      let r = (C * y3 + d).clamp(0.0, 255.0) as u8;
      let g_val = (C * y3 - e - f).clamp(0.0, 255.0) as u8;
      let b = (C * y3 + g).clamp(0.0, 255.0) as u8;
      let idx3 = (y_row_start2 + 2 * uv_x) * 4;
      rgba[idx3] = r;
      rgba[idx3 + 1] = g_val;
      rgba[idx3 + 2] = b;
      rgba[idx3 + 3] = 255;

      // 像素 4: (2*uv_x+1, 2*uv_y+1)
      let y4 = y_plane[y_row_start2 + 2 * uv_x + 1] as f32 - 16.0;
      let r = (C * y4 + d).clamp(0.0, 255.0) as u8;
      let g_val = (C * y4 - e - f).clamp(0.0, 255.0) as u8;
      let b = (C * y4 + g).clamp(0.0, 255.0) as u8;
      let idx4 = (y_row_start2 + 2 * uv_x + 1) * 4;
      rgba[idx4] = r;
      rgba[idx4 + 1] = g_val;
      rgba[idx4 + 2] = b;
      rgba[idx4 + 3] = 255;
    }
  }

  Ok(rgba.into())
}

#[napi]
pub fn copy_rgba_with_rows(
  rgba: &[u8],
  width: u32,
  height: u32,
  bytes_per_row: u32,     // 源数据行字节数（已计算好，含对齐）
  dst_bytes_per_row: u32, // 目标数据行字节数（已计算好，连续）
) -> Result<Buffer> {
  // 转换为 usize 用于索引计算
  let width = width as usize;
  let height = height as usize;
  let bytes_per_row = bytes_per_row as usize;
  let dst_bytes_per_row = dst_bytes_per_row as usize;
  let rgba_data = rgba;

  // 验证输入输出参数合理性
  if dst_bytes_per_row != width * 4 {
    return Err(Error::from_reason(format!(
      "dst_bytes_per_row mismatch: expected {} (width*4), got {}",
      width * 4,
      dst_bytes_per_row
    )));
  }

  // 验证源缓冲区总大小是否足够
  let expected_src_size = bytes_per_row * height;
  if rgba_data.len() != expected_src_size {
    return Err(Error::from_reason(format!(
      "Source RGBA buffer size mismatch: expected {} bytes (bytes_per_row * height), got {}",
      expected_src_size,
      rgba_data.len()
    )));
  }

  // 计算目标缓冲区总大小并初始化
  let png_size = dst_bytes_per_row * height;
  let mut png = vec![0u8; png_size];

  // 逐行复制数据（使用外部传入的行字节数）
  for y in 0..height {
    // 计算当前行的源和目标起始索引
    let src_start = y * bytes_per_row;
    let dst_start = y * dst_bytes_per_row;

    // 验证当前行源数据是否足够（避免越界）
    if src_start + dst_bytes_per_row > rgba_data.len() {
      return Err(Error::from_reason(format!(
        "Row {} data insufficient: need {} bytes, only {} available",
        y,
        dst_bytes_per_row,
        rgba_data.len() - src_start
      )));
    }

    // 复制当前行的有效数据（跳过源数据中的对齐填充）
    png[dst_start..dst_start + dst_bytes_per_row]
      .copy_from_slice(&rgba_data[src_start..src_start + dst_bytes_per_row]);
  }

  Ok(png.into())
}

#[napi]
pub fn merge_audio_arrays(
  pcm_arrays: Vec<Buffer>, // PCM数据数组
  volumes: Vec<f64>,       // 改为f64类型（与JavaScript数字兼容）
) -> Result<Buffer> {
  if pcm_arrays.is_empty() {
    return Ok(Buffer::from(vec![]));
  }

  // 优化：仅一条音频时直接返回（应用音量后）
  if pcm_arrays.len() == 1 {
    let pcm = &pcm_arrays[0];
    let volume = volumes[0];

    // 验证PCM长度为偶数（16位样本）
    if pcm.len() % 2 != 0 {
      return Err(Error::from_reason(
        "PCM数据长度必须为偶数（16位样本）".to_string(),
      ));
    }
    // 验证音量范围
    if volume < 0.0 || volume > 1.0 {
      return Err(Error::from_reason(format!(
        "音量值超出范围（0.0-1.0）: {}",
        volume
      )));
    }

    // 应用音量并返回（无需合并）
    let mut result = pcm.to_vec();
    let sample_count = result.len() / 2;

    for i in 0..sample_count {
      let byte_offset = i * 2;
      // 读取原始样本
      let sample = i16::from_le_bytes(pcm[byte_offset..byte_offset + 2].try_into().unwrap()) as i32;
      // 应用音量并限制范围
      let adjusted = (sample as f64 * volume) as i32;
      let clamped = adjusted.clamp(i16::MIN as i32, i16::MAX as i32) as i16;
      // 写回调整后的值
      let bytes = clamped.to_le_bytes();
      result[byte_offset] = bytes[0];
      result[byte_offset + 1] = bytes[1];
    }

    return Ok(result.into());
  }

  // 验证数组长度匹配
  if pcm_arrays.len() != volumes.len() {
    return Err(Error::from_reason(format!(
      "PCM数组与音量数组长度不匹配: PCM有{}个元素，音量有{}个元素",
      pcm_arrays.len(),
      volumes.len()
    )));
  }

  // 验证PCM长度为偶数（16位样本）
  let frame_length = pcm_arrays[0].len();
  if frame_length % 2 != 0 {
    return Err(Error::from_reason(
      "PCM数据长度必须为偶数（16位样本）".to_string(),
    ));
  }

  // 验证每个PCM长度一致且音量在0.0-1.0范围
  for (i, (pcm, &volume)) in pcm_arrays.iter().zip(volumes.iter()).enumerate() {
    if pcm.len() != frame_length {
      return Err(Error::from_reason(format!(
        "第{}个PCM数据长度不匹配: 预期{}字节，实际{}字节",
        i,
        frame_length,
        pcm.len()
      )));
    }
    if volume < 0.0 || volume > 1.0 {
      return Err(Error::from_reason(format!(
        "第{}个音量值超出范围（0.0-1.0）: {}",
        i, volume
      )));
    }
  }

  // 合并PCM数据
  let sample_count = frame_length / 2;
  let mut merged_data = vec![0u8; frame_length];

  for i in 0..sample_count {
    let byte_offset = i * 2;
    let mut sum = 0i32;

    for (pcm, &volume) in pcm_arrays.iter().zip(volumes.iter()) {
      // 读取16位样本（little-endian）
      let sample = i16::from_le_bytes(pcm[byte_offset..byte_offset + 2].try_into().unwrap()) as i32;

      // 应用音量（f64转f32计算，不影响精度）
      sum += (sample as f32 * volume as f32) as i32;
    }

    // 限制在16位范围
    let clamped = sum.clamp(i16::MIN as i32, i16::MAX as i32) as i16;
    let bytes = clamped.to_le_bytes();
    merged_data[byte_offset] = bytes[0];
    merged_data[byte_offset + 1] = bytes[1];
  }

  Ok(merged_data.into())
}
