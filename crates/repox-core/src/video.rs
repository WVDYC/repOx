//! AI Video Prompt & Continuity Optimizer, FFmpeg Tail-Frame Extractor, and ComfyUI Workflow Compressor.
//!
//! Designed for state-of-the-art video diffusion and flow-matching models
//! (Grok Video, Kling, Sora, Veo, and Wan 2.1), providing:
//! - Dual-output prompt restructuring: an XML `<video_brief>` for LLM planning plus tag-free,
//!   paste-ready video sampler prompts that repeat locked subject/lens/lighting identity per clip.
//! - Denoised center-crop 3x3 discrete Laplacian variance sharpness scoring and real FFmpeg
//!   tail-frame extraction (`extract_sharpest_tail_frame`) with pure-Rust PGM `P5` stream parsing.
//! - Rich ComfyUI JSON graph summarization extracting resolution, frame count, FPS, sampler
//!   hyperparameters, checkpoints/LoRAs, and positive/negative text encodings.

use crate::error::{RepoxError, Result};
use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};
use std::process::Command;

/// Conversational filler prefixes and phrases that dilute diffusion model token attention.
const CONVERSATIONAL_FILLERS: &[&str] = &[
    "please generate a video of",
    "please create a video of",
    "please make a video of",
    "generate a realistic video where",
    "create a realistic video where",
    "create a realistic video of",
    "generate a video where",
    "generate a video of",
    "create a video where",
    "create a video of",
    "make a video where",
    "make a video of",
    "can you generate a video of",
    "can you create a video of",
    "can you make a video of",
    "i would like to see a video of",
    "i want to see a video of",
    "i want a video of",
    "i want to see",
    "i would like to see",
    "show me a video of",
    "render a video of",
    "please generate",
    "please create",
];

/// Low-information stop-words removed when condensing shot action descriptions
/// while preserving prepositions and spatial/motion cues critical for video generation.
const DILUTING_STOP_WORDS: &[&str] = &[
    "just",
    "basically",
    "actually",
    "really",
    "very",
    "quite",
    "literally",
    "simply",
    "maybe",
    "perhaps",
    "sort",
    "kind",
];

/// Explicit lens and optical tokens recognized for `<character_lock>` when present in the user prompt.
/// Ordered longest-first so specific phrases match before shorter substrings.
const LENS_SPECS: &[&str] = &[
    "shallow depth of field",
    "arri alexa",
    "red v-raptor",
    "red komodo",
    "wide-angle",
    "wide angle",
    "anamorphic",
    "panavision",
    "telephoto",
    "100mm",
    "135mm",
    "16mm",
    "24mm",
    "35mm",
    "50mm",
    "85mm",
    "cooke",
    "arri",
    "macro",
    "imax",
    "bokeh",
    "f/1.4",
    "f/1.8",
    "f/2.8",
];

/// Camera movement and framing tokens recognized for `[Camera]` directives.
const CAMERA_MOTION_SPECS: &[&str] = &[
    "extreme close-up",
    "over-the-shoulder",
    "tracking shot",
    "crane shot",
    "medium shot",
    "wide-angle",
    "wide angle",
    "low angle",
    "high angle",
    "close-up",
    "steadicam",
    "handheld",
    "gimbal",
    "aerial",
    "dolly",
    "drone",
    "fpv",
];

/// Lighting descriptors recognized for `<character_lock>` when present in the user prompt.
/// Ordered longest-first so full phrases (e.g. `"neon lighting"`) match before `"neon"`.
const LIGHTING_SPECS: &[&str] = &[
    "volumetric lighting",
    "soft diffused light",
    "cinematic lighting",
    "natural sunlight",
    "dramatic shadows",
    "studio lighting",
    "cyberpunk neon",
    "neon lighting",
    "natural light",
    "high contrast",
    "rim lighting",
    "golden hour",
    "volumetric",
    "chiaroscuro",
    "rim light",
    "blue hour",
    "neon-lit",
    "god rays",
    "overcast",
    "backlit",
    "moonlit",
    "studio",
    "moody",
    "neon",
];

/// Optimizes a raw natural-language video prompt using default options
/// (`shot_duration_secs = None`, `anchor_frame_path = None`).
pub fn optimize_video_prompt(raw_prompt: &str) -> String {
    optimize_video_prompt_with_options(raw_prompt, None, None)
}

/// Optimizes a raw natural-language video prompt into TWO clearly separated sections:
///
/// 1. `# 1. Prompt-Writer Brief (XML for LLM planning)` — compact `<video_brief>` with
///    `<character_lock>` (including ONLY fields explicitly present in the user prompt;
///    never inventing lens or lighting defaults) and `<shot>` blocks.
/// 2. `# 2. Paste-Ready Video Model Prompts (No XML tags — repeat identity per clip)` —
///    tag-free prompts ready to paste directly into Grok Video, Kling, Sora, Veo, or Wan 2.1,
///    repeating the locked identity prefix on every shot and appending
///    `[Conditioning Image: <path>]` when `anchor_frame_path` is provided.
pub fn optimize_video_prompt_with_options(
    raw_prompt: &str,
    shot_duration_secs: Option<u32>,
    anchor_frame_path: Option<&Path>,
) -> String {
    let cleaned = strip_conversational_fillers(raw_prompt);
    let shots = split_into_shots(&cleaned);

    let subject_and_wardrobe = extract_subject_and_wardrobe(&cleaned, &shots);
    let lens_specs = extract_matching_specs(&cleaned, LENS_SPECS);
    let lighting_specs = extract_matching_specs(&cleaned, LIGHTING_SPECS);

    let mut out = String::new();

    // Section 1: XML brief for LLM planning
    out.push_str("# 1. Prompt-Writer Brief (XML for LLM planning)\n");
    out.push_str("<video_brief>\n");
    out.push_str("  <character_lock>\n");
    out.push_str("    subject: ");
    out.push_str(&subject_and_wardrobe);
    out.push('\n');
    if let Some(ref lens) = lens_specs {
        out.push_str("    lens: ");
        out.push_str(lens);
        out.push('\n');
    }
    if let Some(ref lighting) = lighting_specs {
        out.push_str("    lighting: ");
        out.push_str(lighting);
        out.push('\n');
    }
    out.push_str("  </character_lock>\n");

    let mut condensed_shots = Vec::with_capacity(shots.len());
    for (idx, raw_shot) in shots.iter().enumerate() {
        let shot_num = idx + 1;
        let condensed_shot = condense_shot_text(raw_shot);
        let camera_directive = infer_camera_directive(&condensed_shot, lens_specs.as_deref());
        let motion_directive = infer_motion_directive(&condensed_shot);

        let continuity_anchor = if shot_num == 1 {
            if let Some(anchor_path) = anchor_frame_path {
                format!(
                    "[Continuity: anchor={}, lock=character_lock]",
                    anchor_path.display()
                )
            } else {
                "[Continuity: anchor=initial_keyframe, lock=character_lock]".to_string()
            }
        } else {
            "[Continuity: anchor=sharpest_tail_frame, lock=character_lock]".to_string()
        };

        if let Some(secs) = shot_duration_secs {
            out.push_str(&format!(
                "  <shot index=\"{shot_num}\" duration=\"{secs}s\">\n"
            ));
        } else {
            out.push_str(&format!("  <shot index=\"{shot_num}\">\n"));
        }

        if let Some(ref cam) = camera_directive {
            out.push_str("    [Camera: ");
            out.push_str(cam);
            out.push_str("]\n");
        }
        out.push_str("    [Motion: ");
        out.push_str(&motion_directive);
        out.push_str("]\n");
        out.push_str("    ");
        out.push_str(&continuity_anchor);
        out.push_str("\n  </shot>\n");

        condensed_shots.push(condensed_shot);
    }
    out.push_str("</video_brief>\n\n");

    // Section 2: Paste-ready tag-free sampler prompts with repeated identity per clip
    out.push_str("# 2. Paste-Ready Video Model Prompts (No XML tags — repeat identity per clip)\n");
    let identity_prefix = build_identity_prefix(
        &subject_and_wardrobe,
        lens_specs.as_deref(),
        lighting_specs.as_deref(),
    );

    for (idx, condensed_shot) in condensed_shots.iter().enumerate() {
        let shot_num = idx + 1;
        let motion_tail = strip_redundant_subject_prefix(condensed_shot, &subject_and_wardrobe);

        out.push_str(&format!("Shot {shot_num}: {identity_prefix}"));
        if !motion_tail.is_empty() {
            out.push_str(", ");
            out.push_str(&motion_tail);
        }
        if let Some(secs) = shot_duration_secs {
            out.push_str(&format!(" ({secs}s)"));
        }
        if let Some(anchor_path) = anchor_frame_path {
            out.push_str(&format!(" [Conditioning Image: {}]", anchor_path.display()));
        }
        out.push('\n');
    }

    out.trim_end().to_string()
}

/// Computes the denoised center-crop discrete 3x3 Laplacian variance of an 8-bit grayscale
/// frame (`width` x `height`).
///
/// 1. **Center-crop focus**: Evaluates the middle 60% of `width` and `height` (where the primary
///    subject/face typically resides) when `width >= 8 && height >= 8`, falling back to the full
///    frame for tiny test buffers (`< 8x8`). This prevents sharp background edges at the frame
///    periphery from outscoring a motion-blurred subject in the center.
/// 2. **Light 3x3 box/tent denoise pass**: Applies a 3x3 smoothing kernel
///    (`(8 * center + sum(8 neighbors)) / 16`) before computing the discrete 3x3 Laplacian
///    `[0, 1, 0; 1, -4, 1; 0, 1, 0]`, suppressing single-pixel sensor grain and compression
///    artifacts while preserving genuine structural edges.
pub fn compute_frame_sharpness(grayscale: &[u8], width: usize, height: usize) -> f64 {
    if width < 3 || height < 3 {
        return 0.0;
    }
    let Some(required_len) = width.checked_mul(height) else {
        return 0.0;
    };
    if grayscale.len() < required_len {
        return 0.0;
    }

    let (start_x, start_y, crop_w, crop_h) = if width >= 8 && height >= 8 {
        let cw = ((width * 6) / 10).max(3);
        let ch = ((height * 6) / 10).max(3);
        let sx = (width - cw) / 2;
        let sy = (height - ch) / 2;
        (sx, sy, cw, ch)
    } else {
        (0, 0, width, height)
    };

    if crop_w < 3 || crop_h < 3 {
        return 0.0;
    }

    // Pass 1: Light 3x3 box/tent denoise over the crop region
    let mut denoised = vec![0u8; crop_w * crop_h];
    for cy in 0..crop_h {
        let gy = start_y + cy;
        let y0 = gy.saturating_sub(1);
        let y2 = (gy + 1).min(height - 1);

        let r0 = y0 * width;
        let r1 = gy * width;
        let r2 = y2 * width;

        for cx in 0..crop_w {
            let gx = start_x + cx;
            let x0 = gx.saturating_sub(1);
            let x2 = (gx + 1).min(width - 1);

            let neighbors_sum = u32::from(grayscale[r0 + x0])
                + u32::from(grayscale[r0 + gx])
                + u32::from(grayscale[r0 + x2])
                + u32::from(grayscale[r1 + x0])
                + u32::from(grayscale[r1 + x2])
                + u32::from(grayscale[r2 + x0])
                + u32::from(grayscale[r2 + gx])
                + u32::from(grayscale[r2 + x2]);
            let center = u32::from(grayscale[r1 + gx]);
            denoised[cy * crop_w + cx] = ((center * 8 + neighbors_sum + 8) >> 4) as u8;
        }
    }

    // Pass 2: Discrete 3x3 Laplacian variance on denoised center crop
    let interior_w = crop_w - 2;
    let interior_h = crop_h - 2;
    let count = (interior_w * interior_h) as f64;

    let mut sum: i64 = 0;
    let mut sum_sq: i64 = 0;

    for y in 1..(crop_h - 1) {
        let row_prev = (y - 1) * crop_w;
        let row_curr = y * crop_w;
        let row_next = (y + 1) * crop_w;

        for x in 1..(crop_w - 1) {
            let top = i64::from(denoised[row_prev + x]);
            let bottom = i64::from(denoised[row_next + x]);
            let left = i64::from(denoised[row_curr + x - 1]);
            let right = i64::from(denoised[row_curr + x + 1]);
            let center = i64::from(denoised[row_curr + x]);

            let lap = top + bottom + left + right - 4 * center;
            sum += lap;
            sum_sq += lap * lap;
        }
    }

    let mean = (sum as f64) / count;
    let variance = (sum_sq as f64) / count - (mean * mean);
    variance.max(0.0)
}

/// Scans a slice of video frames `(grayscale_bytes, width, height)` and returns the
/// `(frame_index, sharpness_score)` of the sharpest, least motion-blurred frame.
pub fn select_sharpest_frame(frames: &[(&[u8], usize, usize)]) -> Option<(usize, f64)> {
    if frames.is_empty() {
        return None;
    }

    let mut best_idx = 0usize;
    let mut best_score = f64::NEG_INFINITY;

    for (idx, &(buf, width, height)) in frames.iter().enumerate() {
        let score = compute_frame_sharpness(buf, width, height);
        if score > best_score {
            best_score = score;
            best_idx = idx;
        }
    }

    Some((best_idx, best_score.max(0.0)))
}

/// Pure-Rust parser for concatenated binary PGM (`P5 width height 255\n<bytes>`) frames
/// emitted by `ffmpeg -f image2pipe -vcodec pgm -`.
pub fn parse_pgm_stream(bytes: &[u8]) -> Vec<(Vec<u8>, usize, usize)> {
    let mut frames = Vec::new();
    let mut cursor = 0usize;
    let len = bytes.len();

    while cursor + 2 <= len {
        while cursor < len && bytes[cursor].is_ascii_whitespace() {
            cursor += 1;
        }
        if cursor + 2 > len {
            break;
        }
        if &bytes[cursor..cursor + 2] != b"P5" {
            break;
        }
        cursor += 2;

        let Some(width) = read_pgm_header_int(bytes, &mut cursor) else {
            break;
        };
        let Some(height) = read_pgm_header_int(bytes, &mut cursor) else {
            break;
        };
        let Some(maxval) = read_pgm_header_int(bytes, &mut cursor) else {
            break;
        };
        if width == 0 || height == 0 || maxval == 0 || maxval > 255 {
            break;
        }

        // Exactly one whitespace byte separates maxval from the raw 8-bit grayscale raster
        if cursor >= len || !bytes[cursor].is_ascii_whitespace() {
            break;
        }
        cursor += 1;

        let Some(frame_len) = width.checked_mul(height) else {
            break;
        };
        if cursor + frame_len > len {
            break;
        }

        let pixels = bytes[cursor..cursor + frame_len].to_vec();
        frames.push((pixels, width, height));
        cursor += frame_len;
    }

    frames
}

fn skip_pgm_ws_and_comments(bytes: &[u8], cursor: &mut usize) {
    let len = bytes.len();
    while *cursor < len {
        if bytes[*cursor].is_ascii_whitespace() {
            *cursor += 1;
        } else if bytes[*cursor] == b'#' {
            while *cursor < len && bytes[*cursor] != b'\n' {
                *cursor += 1;
            }
        } else {
            break;
        }
    }
}

fn read_pgm_header_int(bytes: &[u8], cursor: &mut usize) -> Option<usize> {
    skip_pgm_ws_and_comments(bytes, cursor);
    let start = *cursor;
    let len = bytes.len();
    let mut val: usize = 0;
    while *cursor < len && bytes[*cursor].is_ascii_digit() {
        val = val
            .checked_mul(10)?
            .checked_add(usize::from(bytes[*cursor] - b'0'))?;
        *cursor += 1;
    }
    if *cursor == start { None } else { Some(val) }
}

/// Probes video duration in seconds via `ffprobe`, returning `None` if unavailable.
fn probe_video_duration(video_path: &Path) -> Option<f64> {
    let output = Command::new("ffprobe")
        .args([
            "-v",
            "error",
            "-show_entries",
            "format=duration",
            "-of",
            "default=noprint_wrappers=1:nokey=1",
        ])
        .arg(video_path)
        .output()
        .ok()?;

    if !output.status.success() {
        return None;
    }

    let text = String::from_utf8_lossy(&output.stdout);
    let dur: f64 = text.trim().parse().ok()?;
    if dur.is_finite() && dur > 0.0 {
        Some(dur)
    } else {
        None
    }
}

/// Decodes a tail window (`-sseof -<window_secs>`) of `video_path` into downscaled 320px-wide
/// PGM frames via `ffmpeg`.
fn decode_tail_pgm_frames(
    video_path: &Path,
    window_secs: f64,
) -> Result<Vec<(Vec<u8>, usize, usize)>> {
    let sseof_arg = format!("-{window_secs:.3}");
    let output = Command::new("ffmpeg")
        .args(["-v", "error", "-sseof", &sseof_arg, "-i"])
        .arg(video_path)
        .args([
            "-vf",
            "scale=320:-1",
            "-f",
            "image2pipe",
            "-vcodec",
            "pgm",
            "-",
        ])
        .output()
        .map_err(|source| RepoxError::Io {
            path: video_path.to_path_buf(),
            source,
        })?;

    if !output.status.success() && output.stdout.is_empty() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(RepoxError::Format(format!(
            "ffmpeg failed to decode tail frames from {}: {}",
            video_path.display(),
            stderr.trim()
        )));
    }

    Ok(parse_pgm_stream(&output.stdout))
}

/// Extracts the sharpest frame from the tail of `video_path` using `ffmpeg`, writes the
/// full-resolution PNG to `output_png` (or `<stem>.anchor.png` next to the video), and returns
/// `(output_png_path, winning_frame_index, sharpness_score)`.
///
/// 1. Probes video duration via `ffprobe` (if available) and decodes the last `0.5s` into
///    320px-wide grayscale PGM frames in memory.
/// 2. Scores each frame with `select_sharpest_frame` (denoised center-crop Laplacian variance).
/// 3. If the maximum sharpness score is below `min_sharpness` (or no frames decoded in 0.5s),
///    warns that the tail is soft and steps back to `-1.5s`.
/// 4. Extracts the winning full-resolution frame to disk as PNG via `ffmpeg`.
pub fn extract_sharpest_tail_frame(
    video_path: &Path,
    output_png: Option<&Path>,
    min_sharpness: f64,
) -> Result<(PathBuf, usize, f64)> {
    if !video_path.exists() {
        return Err(RepoxError::InvalidPath(format!(
            "Video file does not exist: {}",
            video_path.display()
        )));
    }

    let duration_opt = probe_video_duration(video_path);
    let clamp_window = |w: f64| match duration_opt {
        Some(d) if d > 0.05 => w.min(d),
        _ => w,
    };

    let mut window_secs = clamp_window(0.5);
    let mut frames = decode_tail_pgm_frames(video_path, window_secs)?;
    let mut best = select_sharpest_result(&frames);

    if best.is_none_or(|(_, score)| score < min_sharpness) {
        let wider_window = clamp_window(1.5);
        if wider_window > window_secs || frames.is_empty() {
            tracing::warn!(
                "Last {:.2}s tail of {} was soft (max sharpness {:.2} < {:.2}); stepping back to -{:.2}s",
                window_secs,
                video_path.display(),
                best.map_or(0.0, |(_, s)| s),
                min_sharpness,
                wider_window
            );
            let wider_frames = decode_tail_pgm_frames(video_path, wider_window)?;
            if let Some((w_idx, w_score)) = select_sharpest_result(&wider_frames)
                && best.is_none_or(|(_, prev_score)| w_score >= prev_score)
            {
                window_secs = wider_window;
                frames = wider_frames;
                best = Some((w_idx, w_score));
            }
        }
    }

    let Some((best_idx, best_score)) = best else {
        return Err(RepoxError::Format(format!(
            "No decodable video frames found in tail of {}",
            video_path.display()
        )));
    };

    let total_frames = frames.len().max(1);
    let remaining_ratio = ((total_frames - best_idx) as f64) / (total_frames as f64);
    let offset_from_eof = (window_secs * remaining_ratio).max(0.04);
    let sseof_arg = format!("-{offset_from_eof:.3}");

    let out_path = match output_png {
        Some(p) => p.to_path_buf(),
        None => {
            let stem = video_path
                .file_stem()
                .and_then(|s| s.to_str())
                .unwrap_or("video");
            video_path.with_file_name(format!("{stem}.anchor.png"))
        }
    };

    let status = Command::new("ffmpeg")
        .args(["-y", "-v", "error", "-sseof", &sseof_arg, "-i"])
        .arg(video_path)
        .args(["-frames:v", "1"])
        .arg(&out_path)
        .output()
        .map_err(|source| RepoxError::Io {
            path: out_path.clone(),
            source,
        })?;

    // Fallback: if seeking near EOF on a tiny clip produced no file, grab the last frame directly
    if !status.status.success() || !out_path.exists() {
        let fallback = Command::new("ffmpeg")
            .args(["-y", "-v", "error", "-i"])
            .arg(video_path)
            .args(["-frames:v", "1"])
            .arg(&out_path)
            .output()
            .map_err(|source| RepoxError::Io {
                path: out_path.clone(),
                source,
            })?;

        if !fallback.status.success() || !out_path.exists() {
            let stderr = String::from_utf8_lossy(&fallback.stderr);
            return Err(RepoxError::Format(format!(
                "ffmpeg failed to write anchor PNG {}: {}",
                out_path.display(),
                stderr.trim()
            )));
        }
    }

    Ok((out_path, best_idx, best_score))
}

fn select_sharpest_result(frames: &[(Vec<u8>, usize, usize)]) -> Option<(usize, f64)> {
    let refs: Vec<(&[u8], usize, usize)> = frames
        .iter()
        .map(|(buf, w, h)| (buf.as_slice(), *w, *h))
        .collect();
    select_sharpest_frame(&refs)
}

/// Detects a ComfyUI workflow JSON file (either standard UI graph format with `"nodes"` and `"links"`
/// or API format with `"class_type"` entries) and compresses it into a compact, deterministic
/// manifest containing node counts, models/LoRAs, resolution/frame length, sampler settings, and prompts.
///
/// Returns `None` if `json_content` is not a recognized ComfyUI workflow JSON.
pub fn summarize_comfyui_workflow(json_content: &str) -> Option<String> {
    let trimmed = json_content.trim();
    if !trimmed.starts_with('{') || !trimmed.ends_with('}') {
        return None;
    }

    let is_ui_workflow = trimmed.contains("\"nodes\"") && trimmed.contains("\"links\"");
    let is_api_workflow = trimmed.contains("\"class_type\"");

    if !is_ui_workflow && !is_api_workflow {
        return None;
    }

    let mut node_counts: BTreeMap<String, usize> = BTreeMap::new();
    let mut prompts: BTreeSet<String> = BTreeSet::new();
    let mut models: BTreeSet<String> = BTreeSet::new();
    let mut resolution_and_length: BTreeSet<String> = BTreeSet::new();
    let mut sampler_settings: BTreeSet<String> = BTreeSet::new();

    if is_ui_workflow {
        extract_ui_workflow_summary(
            trimmed,
            &mut node_counts,
            &mut prompts,
            &mut models,
            &mut resolution_and_length,
            &mut sampler_settings,
        );
    }

    if is_api_workflow {
        extract_api_workflow_summary(
            trimmed,
            &mut node_counts,
            &mut prompts,
            &mut models,
            &mut resolution_and_length,
            &mut sampler_settings,
        );
    }

    if node_counts.is_empty() {
        return None;
    }

    let total_nodes: usize = node_counts.values().sum();
    let mut out = String::new();
    out.push_str(&format!(
        "# ComfyUI Workflow Manifest ({total_nodes} nodes, {} unique types)\n",
        node_counts.len()
    ));

    out.push_str("## Nodes\n");
    for (node_type, count) in &node_counts {
        out.push_str(&format!("- {node_type} x{count}\n"));
    }

    if !models.is_empty() {
        out.push_str("## Models & Checkpoints\n");
        for model in &models {
            out.push_str(&format!("- {model}\n"));
        }
    }

    if !resolution_and_length.is_empty() {
        out.push_str("## Resolution & Video Length\n");
        for item in &resolution_and_length {
            out.push_str(&format!("- {item}\n"));
        }
    }

    if !prompts.is_empty() {
        out.push_str("## Prompts\n");
        for prompt in &prompts {
            out.push_str(&format!("- \"{prompt}\"\n"));
        }
    }

    if !sampler_settings.is_empty() {
        out.push_str("## Sampler & Latent Config\n");
        for setting in &sampler_settings {
            out.push_str(&format!("- {setting}\n"));
        }
    }

    Some(out)
}

fn strip_conversational_fillers(raw: &str) -> String {
    let mut working = raw.trim().to_string();

    loop {
        let lower = working.to_ascii_lowercase();
        let mut stripped_any = false;
        for filler in CONVERSATIONAL_FILLERS {
            if let Some(rest) = lower.strip_prefix(filler) {
                let offset = working.len() - rest.len();
                working = working[offset..]
                    .trim_start_matches(|c: char| {
                        c.is_whitespace() || c == ',' || c == ':' || c == '-'
                    })
                    .to_string();
                stripped_any = true;
                break;
            }
        }
        if !stripped_any {
            break;
        }
    }

    let lower = working.to_ascii_lowercase();
    for suffix in &[", please.", " please.", ", thank you.", " thank you."] {
        if lower.ends_with(suffix) {
            let keep = working.len() - suffix.len();
            working.truncate(keep);
            break;
        }
    }

    working.trim().to_string()
}

/// Splits cleaned prompt text into shots ONLY on explicit scene/shot markers
/// (`Shot 2:`, `Scene 2:`, `Cut to:`), newlines (`\n`), semicolons (`;`), or sentence
/// boundaries (`.` followed by whitespace or end-of-string, never splitting inside decimals
/// like `f/1.4` or `Wan 2.1`, and never splitting on `"then"`).
fn split_into_shots(cleaned: &str) -> Vec<String> {
    if cleaned.is_empty() {
        return vec!["cinematic establishing shot".to_string()];
    }

    let normalized = normalize_explicit_shot_markers(cleaned);
    let mut shots = Vec::new();
    let mut current = String::new();
    let chars: Vec<char> = normalized.chars().collect();
    let len = chars.len();
    let mut i = 0usize;

    while i < len {
        let ch = chars[i];
        if ch == '\n' || ch == ';' {
            push_cleaned_shot(&mut shots, &mut current);
            i += 1;
            continue;
        }
        if ch == '.' {
            let prev_is_digit = i > 0 && chars[i - 1].is_ascii_digit();
            let next_is_digit = i + 1 < len && chars[i + 1].is_ascii_digit();
            let next_is_boundary = i + 1 >= len || chars[i + 1].is_whitespace();
            if !(prev_is_digit && next_is_digit) && next_is_boundary {
                push_cleaned_shot(&mut shots, &mut current);
                i += 1;
                continue;
            }
        }
        current.push(ch);
        i += 1;
    }
    push_cleaned_shot(&mut shots, &mut current);

    if shots.is_empty() {
        shots.push(cleaned.to_string());
    }

    shots
}

fn normalize_explicit_shot_markers(input: &str) -> String {
    let mut out = input
        .replace(" cut to: ", "\n")
        .replace(" Cut to: ", "\n")
        .replace(" cut to ", "\n")
        .replace(" Cut to ", "\n");

    // Split on explicit `Shot N:` or `Scene N:` markers
    for prefix in ["Shot ", "shot ", "Scene ", "scene "] {
        for num in 1..=20 {
            let marker = format!("{prefix}{num}:");
            if out.contains(&marker) {
                out = out.replace(&marker, "\n");
            }
        }
    }

    out
}

fn push_cleaned_shot(shots: &mut Vec<String>, current: &mut String) {
    let trimmed = current.trim().trim_end_matches('.').trim();
    if !trimmed.is_empty() {
        shots.push(trimmed.to_string());
    }
    current.clear();
}

fn extract_subject_and_wardrobe(cleaned: &str, shots: &[String]) -> String {
    let first_shot = shots.first().map_or(cleaned, String::as_str);
    let condensed = condense_shot_text(first_shot);

    let clause = condensed
        .split([',', ';'])
        .take(2)
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .collect::<Vec<_>>()
        .join(", ");

    if clause.is_empty() {
        "primary subject".to_string()
    } else {
        clause
    }
}

/// Extracts matching specification tokens case-insensitively on word boundaries.
/// Returns `None` when the user prompt contains none of the candidate tokens
/// (never inventing fallback defaults).
fn extract_matching_specs(text: &str, candidates: &[&str]) -> Option<String> {
    let lower = text.to_ascii_lowercase();
    let mut found: Vec<&str> = Vec::new();

    for &spec in candidates {
        let trimmed = spec.trim();
        let needle = trimmed.to_ascii_lowercase();
        if contains_word_bounded(&lower, &needle) {
            let already_covered = found.iter().any(|existing| {
                let ex_lower = existing.to_ascii_lowercase();
                ex_lower == needle || contains_word_bounded(&ex_lower, &needle)
            });
            if !already_covered {
                found.push(trimmed);
            }
        }
    }

    if found.is_empty() {
        None
    } else {
        Some(found.join(", "))
    }
}

fn contains_word_bounded(haystack_lower: &str, needle_lower: &str) -> bool {
    if needle_lower.is_empty() || haystack_lower.len() < needle_lower.len() {
        return false;
    }

    let bytes = haystack_lower.as_bytes();
    let mut start = 0usize;
    while let Some(rel) = haystack_lower[start..].find(needle_lower) {
        let idx = start + rel;
        let end = idx + needle_lower.len();

        let before_ok = idx == 0 || !bytes[idx - 1].is_ascii_alphanumeric();
        let after_ok = end >= bytes.len() || !bytes[end].is_ascii_alphanumeric();

        if before_ok && after_ok {
            return true;
        }
        start = idx + 1;
    }
    false
}

fn condense_shot_text(shot: &str) -> String {
    let mut words = Vec::new();
    for raw_word in shot.split_whitespace() {
        let clean_lower = raw_word
            .trim_matches(|c: char| !c.is_alphanumeric())
            .to_ascii_lowercase();
        if !clean_lower.is_empty() && DILUTING_STOP_WORDS.contains(&clean_lower.as_str()) {
            continue;
        }
        words.push(raw_word);
    }
    words.join(" ")
}

fn infer_camera_directive(shot: &str, locked_lens: Option<&str>) -> Option<String> {
    let lower = shot.to_ascii_lowercase();
    let mut directives: Vec<&str> = Vec::new();

    for &spec in CAMERA_MOTION_SPECS.iter().chain(LENS_SPECS.iter()) {
        let trimmed = spec.trim();
        let needle = trimmed.to_ascii_lowercase();
        if contains_word_bounded(&lower, &needle) && !directives.contains(&trimmed) {
            directives.push(trimmed);
        }
    }

    if !directives.is_empty() {
        Some(directives.join(", "))
    } else {
        locked_lens.map(str::to_string)
    }
}

fn infer_motion_directive(shot: &str) -> String {
    if shot.is_empty() {
        return "natural subject motion".to_string();
    }
    shot.to_string()
}

fn build_identity_prefix(subject: &str, lens: Option<&str>, lighting: Option<&str>) -> String {
    let mut parts = vec![subject.to_string()];
    let subj_lower = subject.to_ascii_lowercase();

    if let Some(l) = lens
        && !subj_lower.contains(&l.to_ascii_lowercase())
    {
        parts.push(l.to_string());
    }
    if let Some(lt) = lighting
        && !subj_lower.contains(&lt.to_ascii_lowercase())
    {
        parts.push(lt.to_string());
    }
    parts.join(", ")
}

fn strip_redundant_subject_prefix(condensed_shot: &str, subject: &str) -> String {
    if let Some(rest) = condensed_shot.strip_prefix(subject) {
        rest.trim_start_matches(|c: char| c.is_whitespace() || c == ',' || c == ';')
            .to_string()
    } else {
        condensed_shot.to_string()
    }
}

fn extract_ui_workflow_summary(
    json: &str,
    node_counts: &mut BTreeMap<String, usize>,
    prompts: &mut BTreeSet<String>,
    models: &mut BTreeSet<String>,
    resolution_and_length: &mut BTreeSet<String>,
    sampler_settings: &mut BTreeSet<String>,
) {
    // Extract `"nodes": [ ... ]` array and inspect each node object
    if let Some(nodes_pos) = json.find("\"nodes\"")
        && let Some(nodes_array) = extract_bracketed_slice(&json[nodes_pos + 7..], '[', ']')
    {
        let mut cursor = 0usize;
        while let Some(rel_open) = nodes_array[cursor..].find('{') {
            let start = cursor + rel_open;
            let Some(node_obj) = extract_bracketed_slice(&nodes_array[start..], '{', '}') else {
                break;
            };
            let node_type = extract_named_string_in_slice(node_obj, "type");
            if let Some(ref t) = node_type
                && !is_primitive_link_type(t)
            {
                *node_counts.entry(t.clone()).or_insert(0) += 1;
            }

            if let Some(w_pos) = node_obj.find("\"widgets_values\"")
                && let Some(array_slice) =
                    extract_bracketed_slice(&node_obj[w_pos + 16..], '[', ']')
            {
                classify_node_widgets(
                    node_type.as_deref().unwrap_or(""),
                    array_slice,
                    prompts,
                    models,
                    resolution_and_length,
                    sampler_settings,
                );
            }

            cursor = start + node_obj.len() + 2;
            if cursor >= nodes_array.len() {
                break;
            }
        }
    }
}

fn extract_api_workflow_summary(
    json: &str,
    node_counts: &mut BTreeMap<String, usize>,
    prompts: &mut BTreeSet<String>,
    models: &mut BTreeSet<String>,
    resolution_and_length: &mut BTreeSet<String>,
    sampler_settings: &mut BTreeSet<String>,
) {
    let mut cursor = 0;
    let class_key = "\"class_type\"";
    while let Some(rel_idx) = json[cursor..].find(class_key) {
        let after_key = cursor + rel_idx + class_key.len();
        if let Some(val) = extract_colon_quoted_string(&json[after_key..]) {
            *node_counts.entry(val).or_insert(0) += 1;
        }
        cursor = after_key;
    }

    // Prompts from CLIPTextEncode / TextEncode ("text", "prompt", "positive", "negative")
    extract_all_named_string_fields(json, "text", prompts, false);
    extract_all_named_string_fields(json, "prompt", prompts, true);

    // Checkpoints / models
    for field in [
        "ckpt_name",
        "unet_name",
        "vae_name",
        "clip_name",
        "lora_name",
    ] {
        extract_all_named_key_value_fields(json, field, models);
    }

    // Resolution & video length
    for field in [
        "width",
        "height",
        "length",
        "frame_count",
        "frames",
        "batch_size",
        "fps",
        "frame_rate",
    ] {
        extract_all_named_scalar_fields(json, field, resolution_and_length);
    }

    // Sampler settings
    for field in [
        "steps",
        "cfg",
        "sampler_name",
        "sampler",
        "scheduler",
        "denoise",
        "seed",
        "noise_seed",
    ] {
        extract_all_named_scalar_fields(json, field, sampler_settings);
    }
}

fn extract_named_string_in_slice(slice: &str, field: &str) -> Option<String> {
    let key = format!("\"{field}\"");
    let pos = slice.find(&key)?;
    extract_colon_quoted_string(&slice[pos + key.len()..])
}

fn extract_all_named_string_fields(
    json: &str,
    field: &str,
    target: &mut BTreeSet<String>,
    filter_short: bool,
) {
    let key = format!("\"{field}\"");
    let mut cursor = 0;
    while let Some(rel_idx) = json[cursor..].find(&key) {
        let after_key = cursor + rel_idx + key.len();
        if let Some(val) = extract_colon_quoted_string(&json[after_key..]) {
            let trimmed = val.trim();
            if (!filter_short || trimmed.len() >= 4) && !trimmed.is_empty() {
                target.insert(trimmed.to_string());
            }
        }
        cursor = after_key;
    }
}

fn extract_all_named_key_value_fields(json: &str, field: &str, target: &mut BTreeSet<String>) {
    let key = format!("\"{field}\"");
    let mut cursor = 0;
    while let Some(rel_idx) = json[cursor..].find(&key) {
        let after_key = cursor + rel_idx + key.len();
        if let Some(val) = extract_colon_quoted_string(&json[after_key..]) {
            let trimmed = val.trim();
            if !trimmed.is_empty() {
                target.insert(format!("{field}: {trimmed}"));
            }
        }
        cursor = after_key;
    }
}

fn extract_all_named_scalar_fields(json: &str, field: &str, target: &mut BTreeSet<String>) {
    let key = format!("\"{field}\"");
    let mut cursor = 0;
    while let Some(rel_idx) = json[cursor..].find(&key) {
        let after_key = cursor + rel_idx + key.len();
        if let Some(val) = extract_colon_scalar_value(&json[after_key..]) {
            target.insert(format!("{field}: {val}"));
        }
        cursor = after_key;
    }
}

fn extract_colon_scalar_value(s: &str) -> Option<String> {
    let trimmed = s.trim_start();
    let after_colon = trimmed.strip_prefix(':')?.trim_start();
    if after_colon.starts_with('[') || after_colon.starts_with('{') {
        // Ignore link references like `["4", 0]` in ComfyUI API inputs
        return None;
    }
    if after_colon.starts_with('"') {
        return extract_colon_quoted_string(s);
    }
    let end = after_colon
        .find(|c: char| c == ',' || c == '}' || c == ']' || c.is_whitespace())
        .unwrap_or(after_colon.len());
    let token = after_colon[..end].trim();
    if token.is_empty() || token == "null" {
        return None;
    }
    if token
        .chars()
        .all(|c| c.is_ascii_digit() || c == '.' || c == '-')
    {
        Some(token.to_string())
    } else {
        None
    }
}

fn classify_node_widgets(
    node_type: &str,
    array_slice: &str,
    prompts: &mut BTreeSet<String>,
    models: &mut BTreeSet<String>,
    resolution_and_length: &mut BTreeSet<String>,
    sampler_settings: &mut BTreeSet<String>,
) {
    let tokens = parse_json_array_scalars(array_slice);
    let type_lower = node_type.to_ascii_lowercase();

    // KSampler widget order: [seed, control_after_generate, steps, cfg, sampler_name, scheduler, denoise]
    if type_lower.contains("ksampler") && tokens.len() >= 7 {
        sampler_settings.insert(format!("seed: {}", tokens[0]));
        sampler_settings.insert(format!("steps: {}", tokens[2]));
        sampler_settings.insert(format!("cfg: {}", tokens[3]));
        sampler_settings.insert(format!("sampler_name: {}", tokens[4]));
        sampler_settings.insert(format!("scheduler: {}", tokens[5]));
        sampler_settings.insert(format!("denoise: {}", tokens[6]));
    } else if (type_lower.contains("latent") || type_lower.contains("empty"))
        && tokens.len() >= 3
        && tokens[0].chars().all(|c| c.is_ascii_digit())
        && tokens[1].chars().all(|c| c.is_ascii_digit())
        && tokens[2].chars().all(|c| c.is_ascii_digit())
    {
        resolution_and_length.insert(format!("width: {}", tokens[0]));
        resolution_and_length.insert(format!("height: {}", tokens[1]));
        resolution_and_length.insert(format!("length: {}", tokens[2]));
    }

    let is_text_encode = type_lower.contains("textencode") || type_lower.contains("prompt");

    for literal in extract_quoted_literals(array_slice) {
        let trimmed = literal.trim();
        if trimmed.is_empty() {
            continue;
        }
        if is_model_filename(trimmed) {
            models.insert(trimmed.to_string());
        } else if is_sampler_or_scheduler_token(trimmed) {
            sampler_settings.insert(trimmed.to_string());
        } else if is_text_encode || (trimmed.len() >= 10 && trimmed.contains(' ')) {
            prompts.insert(trimmed.to_string());
        }
    }
}

fn parse_json_array_scalars(array_slice: &str) -> Vec<String> {
    let mut items = Vec::new();
    let mut current = String::new();
    let mut in_string = false;
    let mut escaped = false;

    for ch in array_slice.chars() {
        if in_string {
            if escaped {
                current.push(ch);
                escaped = false;
            } else if ch == '\\' {
                escaped = true;
            } else if ch == '"' {
                in_string = false;
            } else {
                current.push(ch);
            }
        } else if ch == '"' {
            in_string = true;
        } else if ch == ',' {
            let trimmed = current.trim().to_string();
            if !trimmed.is_empty() {
                items.push(trimmed);
            }
            current.clear();
        } else {
            current.push(ch);
        }
    }
    let tail = current.trim().to_string();
    if !tail.is_empty() {
        items.push(tail);
    }
    items
}

fn is_primitive_link_type(val: &str) -> bool {
    matches!(
        val,
        "MODEL"
            | "CLIP"
            | "VAE"
            | "LATENT"
            | "IMAGE"
            | "MASK"
            | "CONDITIONING"
            | "STRING"
            | "INT"
            | "FLOAT"
            | "CONTROL_NET"
    )
}

fn is_model_filename(s: &str) -> bool {
    let lower = s.to_ascii_lowercase();
    lower.ends_with(".safetensors")
        || lower.ends_with(".ckpt")
        || lower.ends_with(".pt")
        || lower.ends_with(".pth")
        || lower.ends_with(".bin")
        || lower.ends_with(".gguf")
}

fn is_sampler_or_scheduler_token(s: &str) -> bool {
    matches!(
        s,
        "euler"
            | "euler_ancestral"
            | "dpmpp_2m"
            | "dpmpp_2m_sde"
            | "dpmpp_sde"
            | "uni_pc"
            | "ddim"
            | "karras"
            | "normal"
            | "sgm_uniform"
            | "simple"
            | "beta"
    )
}

fn extract_colon_quoted_string(s: &str) -> Option<String> {
    let trimmed = s.trim_start();
    let after_colon = trimmed.strip_prefix(':')?.trim_start();
    let after_quote = after_colon.strip_prefix('"')?;
    let mut out = String::new();
    let mut escaped = false;
    for ch in after_quote.chars() {
        if escaped {
            out.push(ch);
            escaped = false;
        } else if ch == '\\' {
            escaped = true;
        } else if ch == '"' {
            return Some(out);
        } else {
            out.push(ch);
        }
    }
    None
}

fn extract_bracketed_slice(s: &str, open: char, close: char) -> Option<&str> {
    let start = s.find(open)?;
    let rest = &s[start + open.len_utf8()..];
    let mut depth = 1usize;
    let mut in_string = false;
    let mut escaped = false;

    for (idx, ch) in rest.char_indices() {
        if in_string {
            if escaped {
                escaped = false;
            } else if ch == '\\' {
                escaped = true;
            } else if ch == '"' {
                in_string = false;
            }
        } else if ch == '"' {
            in_string = true;
        } else if ch == open {
            depth += 1;
        } else if ch == close {
            depth -= 1;
            if depth == 0 {
                return Some(&rest[..idx]);
            }
        }
    }
    None
}

fn extract_quoted_literals(s: &str) -> Vec<String> {
    let mut results = Vec::new();
    let mut current = String::new();
    let mut in_string = false;
    let mut escaped = false;

    for ch in s.chars() {
        if in_string {
            if escaped {
                current.push(ch);
                escaped = false;
            } else if ch == '\\' {
                escaped = true;
            } else if ch == '"' {
                in_string = false;
                results.push(std::mem::take(&mut current));
            } else {
                current.push(ch);
            }
        } else if ch == '"' {
            in_string = true;
        }
    }

    results
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::Path;

    #[test]
    fn test_optimize_video_prompt_strips_filler_locks_character_and_emits_dual_sections() {
        let raw = "Please generate a video of a cyberpunk courier in a weathered yellow trench coat, \
                   shot on 35mm anamorphic lens with neon lighting, sprinting across a rain-slicked Tokyo rooftop. \
                   Then she leaps over a gap as a drone camera tracks her in slow motion.";

        let optimized = optimize_video_prompt_with_options(
            raw,
            Some(5),
            Some(Path::new("/tmp/clip1.anchor.png")),
        );

        assert!(!optimized.to_ascii_lowercase().contains("please generate"));
        assert!(optimized.contains("# 1. Prompt-Writer Brief (XML for LLM planning)"));
        assert!(optimized.contains(
            "# 2. Paste-Ready Video Model Prompts (No XML tags — repeat identity per clip)"
        ));
        assert!(optimized.contains("<character_lock>"));
        assert!(optimized.contains("cyberpunk courier in a weathered yellow trench coat"));
        assert!(
            optimized.contains("lens: 35mm, anamorphic")
                || optimized.contains("lens: anamorphic, 35mm")
        );
        assert!(optimized.contains("lighting: neon lighting"));
        assert!(optimized.contains("<shot index=\"1\" duration=\"5s\">"));
        assert!(optimized.contains("<shot index=\"2\" duration=\"5s\">"));
        assert!(
            optimized.contains("[Continuity: anchor=sharpest_tail_frame, lock=character_lock]")
        );
        assert!(
            optimized.contains("Shot 1: a cyberpunk courier in a weathered yellow trench coat")
        );
        assert!(
            optimized.contains("Shot 2: a cyberpunk courier in a weathered yellow trench coat")
        );
        assert!(optimized.contains("[Conditioning Image: /tmp/clip1.anchor.png]"));
    }

    #[test]
    fn test_optimize_video_prompt_no_invented_defaults_and_no_split_on_then() {
        let raw = "A woman in a red linen dress turns toward the window then smiles softly";
        let optimized = optimize_video_prompt(raw);

        // Must NOT invent lens or lighting when none were provided
        assert!(!optimized.contains("lens:"));
        assert!(!optimized.contains("lighting:"));
        assert!(!optimized.contains("35mm"));
        assert!(!optimized.contains("volumetric"));
        // Must NOT hardcode duration="5s" when shot_duration_secs is None
        assert!(!optimized.contains("duration=\"5s\""));
        assert!(optimized.contains("<shot index=\"1\">"));
        // Must NOT split on "then" inside a single sentence
        assert!(!optimized.contains("<shot index=\"2\">"));
        assert!(!optimized.contains("Shot 2:"));

        // Case-insensitive word-boundary matching for "cooke" (does not match "cooked")
        let cooked_prompt = "A chef cooked pasta in a busy kitchen";
        let cooked_opt = optimize_video_prompt(cooked_prompt);
        assert!(!cooked_opt.to_ascii_lowercase().contains("lens:"));

        let cooke_prompt = "A violinist on stage, shot on Cooke 85mm lens with moody lighting";
        let cooke_opt = optimize_video_prompt(cooke_prompt);
        assert!(cooke_opt.contains("lens: 85mm, cooke") || cooke_opt.contains("lens: cooke, 85mm"));
        assert!(cooke_opt.contains("lighting: moody"));
    }

    #[test]
    fn test_compute_frame_sharpness_flat_vs_blurry_vs_sharp_and_center_crop() {
        let width = 16;
        let height = 16;

        // Uniform flat frame has 0 Laplacian variance
        let flat = vec![128u8; width * height];
        let flat_score = compute_frame_sharpness(&flat, width, height);
        assert!(flat_score.abs() < 1e-9);

        // Soft gradient / motion-blurred frame has low Laplacian variance
        let mut blurry = vec![0u8; width * height];
        for y in 0..height {
            for x in 0..width {
                blurry[y * width + x] = ((x * 8 + y * 8) % 256) as u8;
            }
        }
        let blurry_score = compute_frame_sharpness(&blurry, width, height);

        // High-contrast checkerboard has very high Laplacian variance
        let mut sharp = vec![0u8; width * height];
        for y in 0..height {
            for x in 0..width {
                sharp[y * width + x] = if (x + y) % 2 == 0 { 255 } else { 0 };
            }
        }
        let sharp_score = compute_frame_sharpness(&sharp, width, height);
        assert!(sharp_score > blurry_score * 10.0);

        // Frame with sharp border noise but flat blurry center must score 0 because center 60% crop is used
        let mut sharp_border_blurry_center = vec![128u8; width * height];
        for y in 0..height {
            for x in 0..width {
                if x < 2 || x >= width - 2 || y < 2 || y >= height - 2 {
                    sharp_border_blurry_center[y * width + x] =
                        if (x + y) % 2 == 0 { 255 } else { 0 };
                }
            }
        }
        let border_only_score = compute_frame_sharpness(&sharp_border_blurry_center, width, height);
        assert!(border_only_score.abs() < 1e-9);
    }

    #[test]
    fn test_select_sharpest_frame_picks_crisp_tail_frame() {
        let width = 6;
        let height = 6;
        let flat = vec![100u8; width * height];
        let mut medium = vec![100u8; width * height];
        medium[2 * width + 2] = 200;

        let mut crisp = vec![0u8; width * height];
        for y in 0..height {
            for x in 0..width {
                crisp[y * width + x] = if (x + y) % 2 == 0 { 250 } else { 10 };
            }
        }

        let frames: Vec<(&[u8], usize, usize)> = vec![
            (&flat, width, height),
            (&crisp, width, height),
            (&medium, width, height),
        ];

        let (best_idx, best_score) = select_sharpest_frame(&frames).unwrap();
        assert_eq!(best_idx, 1);
        assert!(best_score > 1000.0);
        assert!(select_sharpest_frame(&[]).is_none());
    }

    #[test]
    fn test_parse_pgm_stream_multi_frame() {
        let mut stream = Vec::new();
        // Frame 1: 4x3 with comment
        stream.extend_from_slice(b"P5\n# generated by ffmpeg\n4 3\n255\n");
        stream.extend_from_slice(&[10u8; 12]);
        // Frame 2: 2x2 with whitespace byte values (0x20, 0x0A) inside pixel payload
        stream.extend_from_slice(b"P5 2 2 255\n");
        stream.extend_from_slice(&[0x20, 0x0A, 0xFF, 0x00]);

        let frames = parse_pgm_stream(&stream);
        assert_eq!(frames.len(), 2);
        assert_eq!(frames[0].1, 4);
        assert_eq!(frames[0].2, 3);
        assert_eq!(frames[0].0, vec![10u8; 12]);
        assert_eq!(frames[1].1, 2);
        assert_eq!(frames[1].2, 2);
        assert_eq!(frames[1].0, vec![0x20, 0x0A, 0xFF, 0x00]);
    }

    #[test]
    fn test_summarize_comfyui_workflow_ui_and_api_formats() {
        let ui_workflow = r#"{
            "last_node_id": 12,
            "last_link_id": 15,
            "nodes": [
                {
                    "id": 1,
                    "type": "CheckpointLoaderSimple",
                    "pos": [100, 200],
                    "size": {"0": 315, "1": 98},
                    "widgets_values": ["wan2.1_t2v_14B_fp16.safetensors"]
                },
                {
                    "id": 2,
                    "type": "CLIPTextEncode",
                    "pos": [450, 200],
                    "widgets_values": ["cinematic tracking shot of a samurai in bamboo forest, 85mm lens"]
                },
                {
                    "id": 3,
                    "type": "CLIPTextEncode",
                    "pos": [450, 400],
                    "widgets_values": ["blurry, low quality, deformed hands, watermark"]
                },
                {
                    "id": 4,
                    "type": "KSampler",
                    "pos": [800, 250],
                    "widgets_values": [42, "fixed", 30, 7.0, "dpmpp_2m", "karras", 1.0]
                },
                {
                    "id": 5,
                    "type": "EmptyHunyuanLatentVideo",
                    "pos": [450, 600],
                    "widgets_values": [832, 480, 81, 1]
                }
            ],
            "links": [[1, 1, 0, 4, 0, "MODEL"], [2, 2, 0, 4, 1, "CONDITIONING"]]
        }"#;

        let summary = summarize_comfyui_workflow(ui_workflow).unwrap();
        assert!(summary.contains("# ComfyUI Workflow Manifest (5 nodes, 4 unique types)"));
        assert!(summary.contains("- CLIPTextEncode x2"));
        assert!(summary.contains("- CheckpointLoaderSimple x1"));
        assert!(summary.contains("- KSampler x1"));
        assert!(summary.contains("wan2.1_t2v_14B_fp16.safetensors"));
        assert!(
            summary.contains("cinematic tracking shot of a samurai in bamboo forest, 85mm lens")
        );
        assert!(summary.contains("width: 832"));
        assert!(summary.contains("height: 480"));
        assert!(summary.contains("length: 81"));
        assert!(summary.contains("steps: 30"));
        assert!(summary.contains("cfg: 7.0"));
        assert!(summary.contains("seed: 42"));
        assert!(summary.contains("dpmpp_2m"));
        assert!(summary.contains("karras"));

        let api_workflow = r#"{
            "1": {
                "class_type": "UNETLoader",
                "inputs": { "unet_name": "wan2.1_t2v_14B_fp16.safetensors" }
            },
            "2": {
                "class_type": "EmptyWanLatentVideo",
                "inputs": { "width": 1280, "height": 720, "length": 49, "fps": 24 }
            },
            "3": {
                "class_type": "KSampler",
                "inputs": { "seed": 12345, "steps": 25, "cfg": 6.0, "sampler_name": "uni_pc", "scheduler": "simple", "denoise": 1.0 }
            },
            "4": {
                "class_type": "CLIPTextEncode",
                "inputs": { "text": "neon rain in Tokyo" }
            }
        }"#;
        let api_summary = summarize_comfyui_workflow(api_workflow).unwrap();
        assert!(api_summary.contains("unet_name: wan2.1_t2v_14B_fp16.safetensors"));
        assert!(api_summary.contains("width: 1280"));
        assert!(api_summary.contains("height: 720"));
        assert!(api_summary.contains("length: 49"));
        assert!(api_summary.contains("fps: 24"));
        assert!(api_summary.contains("steps: 25"));
        assert!(api_summary.contains("cfg: 6.0"));
        assert!(api_summary.contains("sampler_name: uni_pc"));
        assert!(api_summary.contains("scheduler: simple"));
        assert!(api_summary.contains("seed: 12345"));
        assert!(api_summary.contains("neon rain in Tokyo"));

        assert!(summarize_comfyui_workflow("{\"hello\": \"world\"}").is_none());
    }
}
