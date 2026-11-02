// Copyright (c) the JPEG XL Project Authors. All rights reserved.
// Use of this source code is governed by a BSD-style license in LICENSE.
// Boundary adapters for the actual Rust and C++ implementations. Neither
// adapter provides an alternative implementation of the serialization logic.

#![cfg(feature = "cpp-oracle")]

use jxl::jxl::base::status::{Status, StatusCode, status_code};
use jxl::jxl::dec_bit_reader::BitReader;
use jxl::jxl::enc_aux_out::{AuxOut, LayerType};
use jxl::jxl::enc_bit_writer::BitWriter;
use jxl::jxl::field_encodings::{Fields, U32Distr, U32Enc, bits, bits_offset, val};
use jxl::jxl::fields::{bits_coder, bundle, f16_coder, u32_coder, u64_coder};
use jxl::jxl::headers::{AnimationHeader, PreviewHeader, SizeHeader};
use std::fmt::Write as _;
use std::io::Write;
use std::path::PathBuf;
use std::process::{Command, Stdio};
use std::str::FromStr;

#[path = "common/bundles.rs"]
mod bundles;

#[path = "parity/ac_group_symbols.rs"]
mod ac_group_symbols;

#[path = "parity/block_context.rs"]
mod block_context;

#[path = "parity/dct.rs"]
mod dct;

#[path = "parity/entropy.rs"]
mod entropy;

#[path = "parity/huffman.rs"]
mod huffman;

#[path = "parity/ans.rs"]
mod ans;

#[path = "parity/cluster.rs"]
mod cluster;

#[path = "parity/lz77.rs"]
mod lz77;

#[path = "parity/enc_entropy.rs"]
mod enc_entropy;

#[path = "parity/cms.rs"]
mod cms;

#[path = "parity/metadata.rs"]
mod metadata;

#[path = "parity/frame.rs"]
mod frame;

#[path = "parity/enc_frame.rs"]
mod enc_frame;

#[path = "parity/quant_dc.rs"]
mod quant_dc;

#[path = "parity/quant_ac.rs"]
mod quant_ac;

#[path = "parity/quant_storage.rs"]
mod quant_storage;

#[path = "parity/quantizer.rs"]
mod quantizer;

#[path = "parity/aux_stats.rs"]
mod aux_stats;

#[path = "parity/frame_output.rs"]
mod frame_output;

#[path = "parity/external_image.rs"]
mod external_image;

#[path = "parity/frame_buffers.rs"]
mod frame_buffers;

#[path = "parity/shared_primitives.rs"]
mod shared_primitives;

#[path = "parity/modular_frame_decoder.rs"]
mod modular_frame_decoder;

#[path = "parity/render_pipeline.rs"]
mod render_pipeline;

#[path = "parity/modular_groups.rs"]
mod modular_groups;
#[path = "parity/modular_pixels.rs"]
mod modular_pixels;

#[path = "parity/spot_color.rs"]
mod spot_color;

#[path = "parity/epf.rs"]
mod epf;
#[path = "parity/gaborish_stage.rs"]
mod gaborish_stage;

#[path = "parity/upsampling.rs"]
mod upsampling;

#[path = "parity/output_encoding.rs"]
mod output_encoding;

#[path = "parity/decoder_state.rs"]
mod decoder_state;

#[path = "parity/cms_stage.rs"]
mod cms_stage;

#[path = "parity/xyb_stage.rs"]
mod xyb_stage;

#[path = "parity/tone_mapping.rs"]
mod tone_mapping;

#[path = "parity/decode_frame.rs"]
mod decode_frame;

#[path = "parity/float_output.rs"]
mod float_output;

#[path = "parity/uint_output.rs"]
mod uint_output;

#[path = "parity/whole_image.rs"]
mod whole_image;

#[path = "parity/sample_conversion.rs"]
mod sample_conversion;

#[path = "parity/modular_frame.rs"]
mod modular_frame;

#[path = "parity/modular_frame_state.rs"]
mod modular_frame_state;

#[path = "parity/coeff_orders.rs"]
mod coeff_orders;
#[path = "parity/toc.rs"]
mod toc;

#[path = "parity/image.rs"]
mod image;

#[path = "parity/rct.rs"]
mod rct;

#[path = "parity/learn.rs"]
mod learn;
#[path = "parity/ma.rs"]
mod ma;
#[path = "parity/modular_decode.rs"]
mod modular_decode;
#[path = "parity/predict.rs"]
mod predict;
#[path = "parity/transforms.rs"]
mod transforms;

fn next(seed: &mut u64) -> u64 {
    *seed ^= *seed << 13;
    *seed ^= *seed >> 7;
    *seed ^= *seed << 17;
    *seed
}

fn number<T: FromStr>(args: &mut std::str::SplitWhitespace<'_>) -> T
where
    T::Err: std::fmt::Debug,
{
    args.next().expect("test argument").parse().unwrap()
}

fn hex(bytes: &[u8]) -> String {
    if bytes.is_empty() {
        return "-".to_owned();
    }
    let mut result = String::new();
    for byte in bytes {
        write!(result, "{byte:02x}").unwrap();
    }
    result
}

fn unhex(hex: &str) -> Vec<u8> {
    if hex == "-" {
        return Vec::new();
    }
    (0..hex.len())
        .step_by(2)
        .map(|i| u8::from_str_radix(&hex[i..i + 2], 16).unwrap())
        .collect()
}

fn pad(writer: &mut BitWriter) {
    writer
        .with_max_bits(
            7,
            LayerType::Header,
            None,
            |w, _| {
                w.zero_pad_to_byte();
                Ok(())
            },
            false,
        )
        .unwrap();
}

fn prefix(writer: &mut BitWriter, bits: usize) {
    if bits == 0 {
        return;
    }
    writer
        .with_max_bits(
            bits,
            LayerType::Header,
            None,
            |w, _| {
                w.write(bits, (1 << bits) - 1);
                Ok(())
            },
            false,
        )
        .unwrap();
}

fn reader_trace(args: &mut std::str::SplitWhitespace<'_>) -> String {
    let bytes = unhex(args.next().unwrap());
    let mut reader = BitReader::new(&bytes);
    let mut result = String::new();
    for op in args {
        let value = match op.as_bytes()[0] {
            b'r' => reader.read_bits(op[1..].parse().unwrap()),
            b'p' => reader.peek_bits(op[1..].parse().unwrap()),
            b'c' => {
                reader.consume(op[1..].parse().unwrap());
                0
            }
            b's' => {
                reader.skip_bits(op[1..].parse().unwrap());
                0
            }
            b'f' => {
                reader.refill();
                0
            }
            b'a' => status_code(&reader.jump_to_byte_boundary()) as u64,
            b'b' => status_code(&reader.all_reads_within_bounds()) as u64,
            b'm' => {
                reader.mark_unhealthy();
                0
            }
            _ => panic!("unknown reader operation"),
        };
        write!(result, "{op}={value}@{};", reader.total_bits_consumed()).unwrap();
    }
    write!(result, "close={}", status_code(&reader.close())).unwrap();
    result
}

trait Dimension: Fields + Default {
    fn set(&mut self, x: usize, y: usize) -> Status;
    fn dimensions(&self) -> (usize, usize);
}

impl Dimension for SizeHeader {
    fn set(&mut self, x: usize, y: usize) -> Status {
        self.set(x, y)
    }
    fn dimensions(&self) -> (usize, usize) {
        (self.xsize(), self.ysize())
    }
}

impl Dimension for PreviewHeader {
    fn set(&mut self, x: usize, y: usize) -> Status {
        self.set(x, y)
    }
    fn dimensions(&self) -> (usize, usize) {
        (self.xsize(), self.ysize())
    }
}

fn dimension_header<T: Dimension>(args: &mut std::str::SplitWhitespace<'_>) -> String {
    let (x, y, offset) = (number(args), number(args), number(args));
    let mut header = T::default();
    if let Err(code) = header.set(x, y) {
        return format!("S {}", code as i32);
    }
    let (ext, total) = match bundle::can_encode(&mut header) {
        Ok(sizes) => sizes,
        Err(code) => return format!("C {}", code as i32),
    };
    let mut writer = BitWriter::new();
    prefix(&mut writer, offset);
    let written = bundle::write(&mut header, &mut writer, LayerType::Header, None);
    let bits = writer.bits_written();
    pad(&mut writer);
    let mut decoded = T::default();
    let mut reader = BitReader::new(writer.get_span());
    reader.skip_bits(offset);
    let read = bundle::read(&mut reader, &mut decoded);
    let (dx, dy) = decoded.dimensions();
    let consumed = reader.total_bits_consumed();
    let close = reader.close();
    format!(
        "{ext} {total} {} {bits} {} {} {dx} {dy} {consumed} {}",
        status_code(&written),
        hex(writer.get_span()),
        status_code(&read),
        status_code(&close)
    )
}

fn read_dimension<T: Dimension>(args: &mut std::str::SplitWhitespace<'_>) -> String {
    let bytes = unhex(args.next().unwrap());
    let offset = number(args);
    let mut reader = BitReader::new(&bytes);
    reader.skip_bits(offset);
    let mut header = T::default();
    let read = bundle::read(&mut reader, &mut header);
    let (x, y) = header.dimensions();
    let consumed = reader.total_bits_consumed();
    let close = reader.close();
    format!(
        "{} {x} {y} {consumed} {}",
        status_code(&read),
        status_code(&close)
    )
}

fn primitive(kind: &str, args: &mut std::str::SplitWhitespace<'_>) -> String {
    let mut raw = [0; 4];
    if kind == "u32" {
        for r in &mut raw {
            *r = number(args);
        }
    }
    let width: usize = if kind == "bits" { number(args) } else { 0 };
    let value: u64 = number(args);
    let offset: usize = number(args);
    let enc = U32Enc::new(
        U32Distr { d: raw[0] },
        U32Distr { d: raw[1] },
        U32Distr { d: raw[2] },
        U32Distr { d: raw[3] },
    );
    let fvalue = f32::from_bits(value as u32);
    let mut encoded = 0;
    let can = match kind {
        "bits" => bits_coder::can_encode(width, value as u32, &mut encoded),
        "u32" => u32_coder::can_encode(enc, value as u32, &mut encoded),
        "u64" => u64_coder::can_encode(value, &mut encoded),
        _ => f16_coder::can_encode(fvalue, &mut encoded),
    };
    let mut writer = BitWriter::new();
    let written = writer.with_max_bits(
        96,
        LayerType::Header,
        None,
        |w, _| {
            w.write(offset, (1 << offset) - 1);
            match kind {
                "bits" => bits_coder::write(width, value as u32, w),
                "u32" => u32_coder::write(enc, value as u32, w),
                "u64" => u64_coder::write(value, w),
                _ => f16_coder::write(fvalue, w),
            }
        },
        false,
    );
    let bits = writer.bits_written();
    pad(&mut writer);
    let mut result = format!(
        "{} {encoded} {} {bits} {}",
        status_code(&can),
        status_code(&written),
        hex(writer.get_span())
    );
    if written.is_err() {
        return result;
    }
    let mut reader = BitReader::new(writer.get_span());
    reader.skip_bits(offset);
    let mut read = Ok(());
    let decoded = match kind {
        "bits" => u64::from(bits_coder::read(width, &mut reader)),
        "u32" => u64::from(u32_coder::read(enc, &mut reader)),
        "u64" => u64_coder::read(&mut reader),
        _ => {
            let mut fdecoded = 123.0;
            read = f16_coder::read(&mut reader, &mut fdecoded);
            u64::from(fdecoded.to_bits())
        }
    };
    write!(
        result,
        " {} {decoded} {} {}",
        status_code(&read),
        reader.total_bits_consumed(),
        status_code(&reader.close())
    )
    .unwrap();
    result
}

fn read_half(args: &mut std::str::SplitWhitespace<'_>) -> String {
    let bits: u16 = number(args);
    let bytes = bits.to_le_bytes();
    let mut reader = BitReader::new(&bytes);
    let mut value = 123.0;
    let read = f16_coder::read(&mut reader, &mut value);
    format!(
        "{} {} {} {}",
        status_code(&read),
        value.to_bits(),
        reader.total_bits_consumed(),
        status_code(&reader.close())
    )
}

fn animation(args: &mut std::str::SplitWhitespace<'_>) -> String {
    let mut header = AnimationHeader {
        tps_numerator: number(args),
        tps_denominator: number(args),
        num_loops: number(args),
        have_timecodes: number::<u32>(args) != 0,
    };
    let offset = number(args);
    let (ext, total) = match bundle::can_encode(&mut header) {
        Ok(sizes) => sizes,
        Err(code) => return format!("C {}", code as i32),
    };
    let mut writer = BitWriter::new();
    prefix(&mut writer, offset);
    let written = bundle::write(&mut header, &mut writer, LayerType::Header, None);
    let bits = writer.bits_written();
    pad(&mut writer);
    let mut reader = BitReader::new(writer.get_span());
    reader.skip_bits(offset);
    let mut decoded = AnimationHeader::default();
    let read = bundle::read(&mut reader, &mut decoded);
    format!(
        "{ext} {total} {} {bits} {} {} {} {} {} {} {} {}",
        status_code(&written),
        hex(writer.get_span()),
        status_code(&read),
        decoded.tps_numerator,
        decoded.tps_denominator,
        decoded.num_loops,
        u32::from(decoded.have_timecodes),
        reader.total_bits_consumed(),
        status_code(&reader.close())
    )
}

fn nested(args: &mut std::str::SplitWhitespace<'_>) -> String {
    let (offset, width, value): (usize, usize, u64) = (number(args), number(args), number(args));
    let fail = number::<u32>(args) != 0;
    let histogram = number::<u32>(args) != 0;
    let mut aux = AuxOut::default();
    let mut writer = BitWriter::new();
    let mut inner: Status = Ok(());
    let outer = writer.with_max_bits(
        16,
        LayerType::Header,
        Some(&mut aux),
        |w, aux| {
            w.write(offset, (1 << offset) - 1);
            inner = w.with_max_bits(
                64,
                LayerType::Ac,
                aux,
                |w, _| {
                    w.write(width, value);
                    if fail {
                        Err(StatusCode::GenericError)
                    } else {
                        Ok(())
                    }
                },
                histogram,
            );
            inner?;
            w.write(3, 5);
            Ok(())
        },
        histogram,
    );
    let bits = writer.bits_written();
    pad(&mut writer);
    writer.shrink().unwrap();
    format!(
        "{} {} {bits} {} {} {} {} {} {}",
        status_code(&outer),
        status_code(&inner),
        aux.layer(LayerType::Header).total_bits,
        aux.layer(LayerType::Header).histogram_bits,
        aux.layer(LayerType::Ac).total_bits,
        aux.layer(LayerType::Ac).histogram_bits,
        aux.total_bits(),
        hex(writer.get_span())
    )
}

trait BundleObservation: Fields + Default {
    fn observation(&self) -> String;
}

impl BundleObservation for bundles::OldBundle {
    fn observation(&self) -> String {
        format!(
            "{} {} {} {} - - -",
            self.old_small,
            self.old_f.to_bits(),
            self.old_large,
            self.extensions
        )
    }
}

impl BundleObservation for bundles::NewBundle {
    fn observation(&self) -> String {
        format!(
            "{} {} {} {} {} {} {}",
            self.old_small,
            self.old_f.to_bits(),
            self.old_large,
            self.extensions,
            self.new_small,
            self.new_f.to_bits(),
            self.new_large
        )
    }
}

fn read_bundle<T: BundleObservation>(bytes: &[u8], offset: usize) -> String {
    let mut decoded = T::default();
    let mut reader = BitReader::new(bytes);
    reader.skip_bits(offset);
    let read = bundle::read(&mut reader, &mut decoded);
    let consumed = reader.total_bits_consumed();
    let sentinel = reader.read_bits(20);
    let close = reader.close();
    let mut checked = T::default();
    let mut probe = BitReader::new(bytes);
    probe.skip_bits(offset);
    let can_read = bundle::can_read(&mut probe, &mut checked);
    format!(
        " {} {} {consumed} {sentinel} {} {} {} {}",
        status_code(&read),
        decoded.observation(),
        status_code(&close),
        u32::from(can_read),
        probe.total_bits_consumed(),
        status_code(&probe.close())
    )
}

fn bundle_case(args: &mut std::str::SplitWhitespace<'_>) -> String {
    let mut fields = bundles::NewBundle {
        extensions: number(args),
        old_small: number(args),
        old_f: f32::from_bits(number(args)),
        old_large: number(args),
        new_small: number(args),
        new_f: f32::from_bits(number(args)),
        new_large: number(args),
    };
    let decode_new = number::<u32>(args) != 0;
    let offset = number(args);
    let limit = number::<usize>(args);
    let (ext, total) = match bundle::can_encode(&mut fields) {
        Ok(sizes) => sizes,
        Err(code) => return format!("C {}", code as i32),
    };
    let mut writer = BitWriter::new();
    prefix(&mut writer, offset);
    let written = bundle::write(&mut fields, &mut writer, LayerType::Header, None);
    let bits = writer.bits_written();
    writer
        .with_max_bits(
            32,
            LayerType::Header,
            None,
            |w, _| {
                w.write(20, 0xA55A);
                w.zero_pad_to_byte();
                Ok(())
            },
            false,
        )
        .unwrap();
    let mut result = format!(
        "{ext} {total} {} {bits} {}",
        status_code(&written),
        hex(writer.get_span())
    );
    let bytes = &writer.get_span()[..limit.min(writer.get_span().len())];
    result.push_str(&if decode_new {
        read_bundle::<bundles::NewBundle>(bytes, offset)
    } else {
        read_bundle::<bundles::OldBundle>(bytes, offset)
    });
    result
}

fn default_case(args: &mut std::str::SplitWhitespace<'_>) -> String {
    let mut fields = bundles::DefaultedBundle {
        value: number(args),
        ..Default::default()
    };
    let (x, y, offset, limit): (usize, usize, usize, usize) =
        (number(args), number(args), number(args), number(args));
    if (x != 0 || y != 0) && fields.size.set(x, y).is_err() {
        return "S 1".to_owned();
    }
    let (ext, total) = match bundle::can_encode(&mut fields) {
        Ok(sizes) => sizes,
        Err(code) => return format!("C {}", code as i32),
    };
    let mut writer = BitWriter::new();
    prefix(&mut writer, offset);
    let written = bundle::write(&mut fields, &mut writer, LayerType::Header, None);
    let bits = writer.bits_written();
    pad(&mut writer);
    let bytes = writer.get_span();
    let mut decoded = bundles::DefaultedBundle::default();
    let mut reader = BitReader::new(&bytes[..limit.min(bytes.len())]);
    reader.skip_bits(offset);
    let read = bundle::read(&mut reader, &mut decoded);
    format!(
        "{ext} {total} {} {bits} {} {} {} {} {} {} {} {} {}",
        status_code(&written),
        hex(bytes),
        status_code(&read),
        u32::from(fields.all_default),
        u32::from(decoded.all_default),
        decoded.value,
        decoded.size.xsize(),
        decoded.size.ysize(),
        reader.total_bits_consumed(),
        status_code(&reader.close())
    )
}

fn append_case(args: &mut std::str::SplitWhitespace<'_>) -> String {
    let (offset, width, value): (usize, usize, u64) = (number(args), number(args), number(args));
    let suffix = unhex(args.next().unwrap());
    let mut writer = BitWriter::new();
    let mut other = BitWriter::new();
    prefix(&mut writer, offset);
    other
        .with_max_bits(
            64,
            LayerType::Header,
            None,
            |w, _| {
                w.write(width, value);
                Ok(())
            },
            false,
        )
        .unwrap();
    let unaligned = writer.append_unaligned(&other);
    pad(&mut writer);
    let aligned = writer.append_byte_aligned(&suffix);
    let mut parts = Vec::new();
    for n in [0, 1, 2] {
        let mut part = BitWriter::new();
        part.with_max_bits(
            24,
            LayerType::Header,
            None,
            |w, _| {
                for i in 0..n {
                    w.write(8, 0xEF - i);
                }
                Ok(())
            },
            false,
        )
        .unwrap();
        parts.push(part);
    }
    let group = writer.append_byte_aligned_writers(&parts.iter().collect::<Vec<_>>());
    writer
        .with_max_bits(
            16,
            LayerType::Header,
            None,
            |w, _| {
                w.write(13, 0x1ABC);
                w.zero_pad_to_byte();
                Ok(())
            },
            false,
        )
        .unwrap();
    writer.shrink().unwrap();
    format!(
        "{} {} {} {} {}",
        status_code(&unaligned),
        status_code(&aligned),
        status_code(&group),
        writer.bits_written(),
        hex(writer.get_span())
    )
}

fn observe(request: &str) -> String {
    let mut args = request.split_whitespace();
    match args.next().unwrap() {
        "reader" => reader_trace(&mut args),
        "size" => dimension_header::<SizeHeader>(&mut args),
        "preview" => dimension_header::<PreviewHeader>(&mut args),
        "read-size" => read_dimension::<SizeHeader>(&mut args),
        "read-preview" => read_dimension::<PreviewHeader>(&mut args),
        "f16-read" => read_half(&mut args),
        "animation" => animation(&mut args),
        "nested" => nested(&mut args),
        "bundle" => bundle_case(&mut args),
        "default" => default_case(&mut args),
        "append" => append_case(&mut args),
        kind if matches!(kind, "bits" | "u32" | "u64" | "f32") => primitive(kind, &mut args),
        kind if kind.starts_with("huffman") => huffman::observe(kind, &mut args),
        kind if ac_group_symbols::handles(kind) => ac_group_symbols::observe(kind, &mut args),
        kind if ans::handles(kind) => ans::observe(kind, &mut args),
        kind if cluster::handles(kind) => cluster::observe(kind, &mut args),
        kind if lz77::handles(kind) => lz77::observe(kind, &mut args),
        kind if enc_entropy::handles(kind) => enc_entropy::observe(kind, &mut args),
        kind if cms::handles(kind) => cms::observe(kind, &mut args),
        kind if metadata::handles(kind) => metadata::observe(kind, &mut args),
        kind if frame::handles(kind) => frame::observe(kind, &mut args),
        kind if enc_frame::handles(kind) => enc_frame::observe(kind, &mut args),
        kind if quant_dc::handles(kind) => quant_dc::observe(kind, &mut args),
        kind if quant_ac::handles(kind) => quant_ac::observe(kind, &mut args),
        kind if quant_storage::handles(kind) => quant_storage::observe(kind, &mut args),
        kind if quantizer::handles(kind) => quantizer::observe(kind, &mut args),
        kind if aux_stats::handles(kind) => aux_stats::observe(kind, &mut args),
        kind if frame_output::handles(kind) => frame_output::observe(kind, &mut args),
        kind if external_image::handles(kind) => external_image::observe(kind, &mut args),
        kind if frame_buffers::handles(kind) => frame_buffers::observe(kind, &mut args),
        kind if shared_primitives::handles(kind) => shared_primitives::observe(kind, &mut args),
        kind if modular_frame_decoder::handles(kind) => {
            modular_frame_decoder::observe(kind, &mut args)
        }
        kind if render_pipeline::handles(kind) => render_pipeline::observe(kind, &mut args),
        kind if modular_groups::handles(kind) => modular_groups::observe(kind, &mut args),
        kind if dct::handles(kind) => dct::observe(kind, &mut args),
        kind if block_context::handles(kind) => block_context::observe(kind, &mut args),
        kind if spot_color::handles(kind) => spot_color::observe(kind, &mut args),
        kind if epf::handles(kind) => epf::observe(kind, &mut args),
        kind if gaborish_stage::handles(kind) => gaborish_stage::observe(kind, &mut args),
        kind if upsampling::handles(kind) => upsampling::observe(kind, &mut args),
        kind if output_encoding::handles(kind) => output_encoding::observe(kind, &mut args),
        kind if decoder_state::handles(kind) => decoder_state::observe(kind, &mut args),
        kind if cms_stage::handles(kind) => cms_stage::observe(kind, &mut args),
        kind if xyb_stage::handles(kind) => xyb_stage::observe(kind, &mut args),
        kind if tone_mapping::handles(kind) => tone_mapping::observe(kind, &mut args),
        kind if decode_frame::handles(kind) => decode_frame::observe(kind, &mut args),
        kind if uint_output::handles(kind) => uint_output::observe(kind, &mut args),
        kind if float_output::handles(kind) => float_output::observe(kind, &mut args),
        kind if vardct_dc::handles(kind) => vardct_dc::observe(kind, &mut args),
        kind if group_schedule::handles(kind) => group_schedule::observe(kind, &mut args),
        kind if group_render::handles(kind) => group_render::observe(kind, &mut args),
        kind if dequant::handles(kind) => dequant::observe(kind, &mut args),
        kind if vardct_image::handles(kind) => vardct_image::observe(kind, &mut args),
        kind if whole_image::handles(kind) => whole_image::observe(kind, &mut args),
        kind if sample_conversion::handles(kind) => sample_conversion::observe(kind, &mut args),
        kind if modular_frame::handles(kind) => modular_frame::observe(kind, &mut args),
        kind if modular_frame_state::handles(kind) => modular_frame_state::observe(kind, &mut args),
        kind if toc::handles(kind) => toc::observe(kind, &mut args),
        kind if coeff_orders::handles(kind) => coeff_orders::observe(kind, &mut args),
        kind if image::handles(kind) => image::observe(kind, &mut args),
        kind if rct::handles(kind) => rct::observe(kind, &mut args),
        kind if predict::handles(kind) => predict::observe(kind, &mut args),
        kind if ma::handles(kind) => ma::observe(kind, &mut args),
        kind if transforms::handles(kind) => transforms::observe(kind, &mut args),
        kind if modular_decode::handles(kind) => modular_decode::observe(kind, &mut args),
        kind if learn::handles(kind) => learn::observe(kind, &mut args),
        kind => entropy::observe(kind, &mut args),
    }
}

fn original_observations(requests: &[String]) -> Vec<String> {
    let oracle = std::env::var_os("JXL_CPP_ORACLE")
        .map(PathBuf::from)
        .unwrap_or_else(|| {
            PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("target/oracle/jxl_oracle")
        });
    assert!(
        oracle.is_file(),
        "C++ oracle missing: run python3 tools/check.py; expected {}",
        oracle.display()
    );
    let child = Command::new(oracle)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("start C++ oracle");
    let mut child = child;
    let mut input = child.stdin.take().unwrap();
    let mut lines = requests.join("\n");
    lines.push('\n');
    // Drain stdout while feeding stdin so large corpora cannot deadlock on
    // pipe capacity. The oracle is a separate process, not a runtime fallback.
    let feed = std::thread::spawn(move || input.write_all(lines.as_bytes()));
    let output = child.wait_with_output().expect("wait for C++ oracle");
    let fed = feed.join().expect("join oracle input writer");
    assert!(
        output.status.success(),
        "oracle failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    fed.expect("write all oracle requests");
    let text = String::from_utf8(output.stdout).expect("UTF-8 oracle protocol");
    let observations: Vec<_> = text.lines().map(str::to_owned).collect();
    assert_eq!(observations.len(), requests.len(), "oracle response count");
    observations
}

fn compare(requests: Vec<String>) {
    let observations = original_observations(&requests);
    for (request, cpp) in requests.iter().zip(observations) {
        assert_eq!(observe(request), cpp, "request: {request}");
    }
    println!("{} C++/Rust differential cases matched", requests.len());
}

#[test]
fn reader_refills_zero_extension_and_checkpoints() {
    let mut cases = Vec::new();
    let mut seed = 42;
    for len in 0..40 {
        let bytes: Vec<_> = (0..len).map(|_| next(&mut seed) as u8).collect();
        let bytes = hex(&bytes);
        for offset in 0..8 {
            for width in 0..=56 {
                cases.push(format!("reader {bytes} r{offset} r{width} f p56 c0 r0 b"));
            }
            for skip in [0, 1, 7, 8, 55, 56, 57, 63, 64, 127, 128, 4096, usize::MAX] {
                cases.push(format!(
                    "reader {bytes} r{offset} s{skip} r7 p16 a b m r3 b"
                ));
            }
        }
    }
    cases.push("reader ff r9".into());
    cases.push("reader ff r9 b".into());
    cases.push("reader ff r9 b r1".into());
    cases.push("reader 81 r1 a".into());
    compare(cases);
}

#[test]
fn all_binary16_patterns_match_exact_float_bits_and_status() {
    compare(
        (0..=u16::MAX)
            .map(|bits| format!("f16-read {bits}"))
            .collect(),
    );
}

#[test]
fn binary16_writing_preserves_truncation_signed_zero_and_rejection() {
    let mut cases = Vec::new();
    for bits in 0..=u16::MAX {
        let bytes = bits.to_le_bytes();
        let mut reader = BitReader::new(&bytes);
        let mut value = 0.0;
        let read = f16_coder::read(&mut reader, &mut value);
        reader.close().unwrap();
        if read.is_ok() {
            cases.push(format!("f32 {} {}", value.to_bits(), bits % 8));
        }
    }
    let mut seed = 73;
    for _ in 0..8192 {
        let bits = next(&mut seed) as u32;
        cases.push(format!("f32 {bits} {}", bits % 8));
    }
    for value in [
        0.0f32,
        -0.0,
        65504.0,
        65504.01,
        65505.0,
        -65505.0,
        f32::NAN,
        f32::INFINITY,
        f32::NEG_INFINITY,
        f32::MIN_POSITIVE,
        f32::from_bits(1),
    ] {
        for offset in 0..8 {
            cases.push(format!("f32 {} {offset}", value.to_bits()));
        }
    }
    compare(cases);
}

#[test]
fn integer_coders_match_boundaries_selectors_and_unaligned_streams() {
    let mut cases = Vec::new();
    for width in 0..=32 {
        let limit = (1u64 << width) - 1;
        for value in [
            0,
            1,
            limit / 2,
            limit,
            (limit + 1).min(u32::MAX as u64),
            u32::MAX as u64,
        ] {
            for offset in 0..8 {
                cases.push(format!("bits {width} {value} {offset}"));
            }
        }
    }
    let distributions = [
        [val(0), bits(4), val(0x7FFF_FFFF), bits(32)],
        [
            val(0),
            bits_offset(1, 1),
            bits_offset(2, 3),
            bits_offset(8, 8),
        ],
        [bits(32), bits(32), val(3), val(3)],
        [val(1), val(1), val(1), val(1)],
        [bits_offset(30, 1); 4],
        [bits_offset(32, 0x03FF_FFFF); 4],
    ];
    let mut seed = 90210;
    for distr in distributions {
        let enc = format!(
            "{} {} {} {}",
            distr[0].d, distr[1].d, distr[2].d, distr[3].d
        );
        let mut values = vec![
            0,
            1,
            2,
            3,
            7,
            8,
            15,
            16,
            255,
            256,
            0x03FF_FFFE,
            0x03FF_FFFF,
            0x0400_0000,
            0x7FFF_FFFF,
            0x8000_0000,
            u32::MAX,
        ];
        for _ in 0..1024 {
            values.push(next(&mut seed) as u32);
        }
        for (i, value) in values.into_iter().enumerate() {
            cases.push(format!("u32 {enc} {value} {}", i % 8));
        }
    }
    for value in 0..=4096 {
        cases.push(format!("u64 {value} {}", value % 8));
    }
    for power in 0..64 {
        let value = 1u64 << power;
        for value in [value - 1, value, value.saturating_add(1)] {
            for offset in 0..8 {
                cases.push(format!("u64 {value} {offset}"));
            }
        }
    }
    for i in 0..2048 {
        cases.push(format!("u64 {} {}", next(&mut seed), i % 8));
    }
    cases.push(format!("u64 {} 7", u64::MAX));
    compare(cases);
}

#[test]
fn dimension_headers_match_field_order_aspect_ratios_and_limits() {
    let mut cases = Vec::new();
    let values = [
        0,
        1,
        7,
        8,
        9,
        16,
        31,
        32,
        127,
        128,
        255,
        256,
        257,
        511,
        512,
        513,
        8192,
        8193,
        262144,
        262145,
        1 << 30,
        (1 << 30) + 1,
        u32::MAX as usize,
    ];
    for x in values {
        for y in values {
            for offset in 0..8 {
                cases.push(format!("size {x} {y} {offset}"));
            }
        }
    }
    let mut seed = 98;
    for i in 0..4096 {
        let x = next(&mut seed) % (1 << 30) + 1;
        let y = next(&mut seed) % (1 << 30) + 1;
        cases.push(format!("size {x} {y} {}", i % 8));
    }
    for i in 1..4360 {
        cases.push(format!("preview {i} {i} {}", i % 8));
    }
    for x in [
        0, 1, 7, 8, 16, 32, 127, 128, 256, 257, 4352, 4359, 4360, 5440,
    ] {
        for y in [0, 1, 8, 9, 16, 128, 256, 4352, 4360, 5440] {
            for offset in 0..8 {
                cases.push(format!("preview {x} {y} {offset}"));
            }
        }
    }
    #[cfg(target_pointer_width = "64")]
    {
        cases.push(format!("size {} 8 0", u32::MAX as u64 + 1));
        cases.push(format!("preview {} 8 0", u32::MAX as u64 + 1));
    }
    compare(cases);
}

#[test]
fn truncated_and_arbitrary_dimension_headers_preserve_partial_state() {
    let mut cases = Vec::new();
    let mut seed = 55537;
    for len in 0..12 {
        for _ in 0..64 {
            let bytes: Vec<_> = (0..len).map(|_| next(&mut seed) as u8).collect();
            for offset in 0..8 {
                cases.push(format!("read-size {} {offset}", hex(&bytes)));
                cases.push(format!("read-preview {} {offset}", hex(&bytes)));
            }
        }
    }
    // Every byte truncation of representative real serialized headers.
    for (x, y) in [(8, 8), (1920, 1080), (12345, 67890), (1 << 30, 1 << 30)] {
        let mut header = SizeHeader::default();
        header.set(x, y).unwrap();
        let mut writer = BitWriter::new();
        bundle::write(&mut header, &mut writer, LayerType::Header, None).unwrap();
        pad(&mut writer);
        for len in 0..=writer.get_span().len() {
            cases.push(format!("read-size {} 0", hex(&writer.get_span()[..len])));
        }
    }
    compare(cases);
}

#[test]
fn animation_headers_preserve_rational_ticks_and_loop_counts() {
    let mut cases = Vec::new();
    for num in [
        0,
        1,
        100,
        1000,
        1024,
        1025,
        30000,
        1 << 30,
        (1 << 30) + 1,
        u32::MAX,
    ] {
        for den in [0, 1, 256, 1001, 1024, 1025, u32::MAX] {
            for loops in [0, 7, 8, 65535, 65536, u32::MAX] {
                for flag in 0..2 {
                    cases.push(format!(
                        "animation {num} {den} {loops} {flag} {}",
                        (num % 8) as usize
                    ));
                }
            }
        }
    }
    compare(cases);
}

#[test]
fn nested_allotment_charging_histograms_and_failure_reclamation() {
    let mut cases = Vec::new();
    for prefix in 0..8 {
        for width in 0..=56 {
            let value = (1u64 << width) - 1;
            for fail in 0..2 {
                for histogram in 0..2 {
                    cases.push(format!(
                        "nested {prefix} {width} {value} {fail} {histogram}"
                    ));
                }
            }
        }
    }
    compare(cases);
}

#[test]
fn extension_bundles_preserve_compatibility_skipping_and_retryability() {
    let mut cases = Vec::new();
    let mut seed = 9173;
    for ext in [0u64, 1, 2, 3, 4, 7, 1 << 63, u64::MAX] {
        for old_small in [0, 1, 3, 7, 15, 16] {
            for decode_new in 0..2 {
                for offset in 0..8 {
                    for limit in [0, 1, 3, 7, 15, usize::MAX] {
                        let old_large = next(&mut seed) as u32;
                        let new_large = next(&mut seed) as u32;
                        cases.push(format!("bundle {ext} {old_small} {} {old_large} 3 {} {new_large} {decode_new} {offset} {limit}", 3.75f32.to_bits(), 999.0f32.to_bits()));
                    }
                }
            }
        }
    }
    compare(cases);
}

#[test]
fn all_default_control_flow_and_nested_resets_match_even_when_truncated() {
    let mut cases = Vec::new();
    for value in [0, 7, 15, 16] {
        for (x, y) in [(0, 0), (1, 1), (8, 8), (1920, 1080)] {
            for offset in 0..8 {
                for limit in [0, 1, 2, 3, usize::MAX] {
                    cases.push(format!("default {value} {x} {y} {offset} {limit}"));
                }
            }
        }
    }
    compare(cases);
}

#[test]
fn appends_preserve_padding_and_following_writes() {
    let mut cases = Vec::new();
    let mut seed = 8173;
    for offset in 0..8 {
        for width in 0..=56 {
            let value = next(&mut seed) & ((1 << width) - 1);
            for suffix in ["-", "00", "0180ff", "0102030405060708090a0b0c0d0e0f10"] {
                cases.push(format!("append {offset} {width} {value} {suffix}"));
            }
        }
    }
    compare(cases);
}

#[path = "parity/vardct_image.rs"]
mod vardct_image;

#[path = "parity/vardct_dc.rs"]
mod vardct_dc;

#[path = "parity/group_schedule.rs"]
mod group_schedule;

#[path = "parity/dequant.rs"]
mod dequant;

#[path = "parity/group_render.rs"]
mod group_render;
