// Copyright (c) the JPEG XL Project Authors. All rights reserved.
// Use of this source code is governed by a BSD-style license in LICENSE.
use super::*;
use jxl::jxl::render_pipeline::{
    render_pipeline_stage::{
        RENDER_PIPELINE_X_OFFSET as OFFSET, Row, RowBuffers, RowInfo, RowStorage,
        WriteToImage3FStage,
    },
    simple_render_pipeline::SimpleRenderPipelineBuilder,
    stage_spot::{get_spot_color_stage, get_spot_color_stage_cells},
};
use jxl::jxl::{
    frame_dimensions::FrameDimensions,
    image::{Image3, Plane},
};
use std::cell::Cell;
pub(super) fn handles(s: &str) -> bool {
    s.starts_with("spot-")
}
fn color(a: &mut std::str::SplitWhitespace<'_>) -> [f32; 4] {
    std::array::from_fn(|_| f32::from_bits(number(a)))
}
fn print_color(out: &mut String, a: &[f32; 4]) {
    for v in a {
        write!(out, " {}", v.to_bits()).unwrap();
    }
}
pub(super) fn observe(kind: &str, args: &mut std::str::SplitWhitespace<'_>) -> String {
    if kind == "spot-simple" {
        let w = number(args);
        let h = number(args);
        let count = number::<usize>(args);
        let threads = number::<usize>(args);
        let mode = number(args);
        let reverse = number::<u32>(args) != 0;
        let mut offsets = Vec::new();
        let mut colors = Vec::new();
        let mut changed = Vec::new();
        for _ in 0..count {
            offsets.push(number::<usize>(args));
            colors.push(color(args).map(Cell::new));
            changed.push(color(args));
        }
        let mut dim = FrameDimensions::default();
        dim.set(w, h, 0, 0, 0, true, 1);
        let mut image = Image3::default();
        let mut out = String::new();
        {
            let mut builder = SimpleRenderPipelineBuilder::new(7);
            for i in 0..count {
                let s = builder.add_stage(Some(get_spot_color_stage_cells(offsets[i], &colors[i])));
                write!(out, " B {}", status_code(&s)).unwrap();
                if s.is_err() {
                    return out;
                }
            }
            for i in 0..count {
                for c in 0..4 {
                    colors[i][c].set(changed[i][c]);
                }
            }
            builder
                .add_stage(Some(Box::new(WriteToImage3FStage::new(&mut image))))
                .unwrap();
            let mut pipeline = builder.finalize(dim).unwrap();
            let mut s = pipeline.prepare_for_threads(threads, true);
            write!(out, " T {}", status_code(&s)).unwrap();
            if s.is_ok() {
                for i in 0..dim.num_groups {
                    let g = if reverse { dim.num_groups - 1 - i } else { i };
                    let mut input = pipeline.get_input_buffers(g, i % threads);
                    for c in 0..7 {
                        let (plane, r) = input.get_buffer_mut(c);
                        for y in 0..r.ysize() {
                            for x in 0..r.xsize() {
                                plane.row_mut(r.y0() + y)[r.x0() + x] = render_pipeline::value(
                                    c,
                                    r.x0() + x - OFFSET,
                                    r.y0() + y - OFFSET,
                                    0,
                                    mode,
                                );
                            }
                        }
                    }
                    s = input.done();
                    write!(out, " D {}", status_code(&s)).unwrap();
                    if s.is_err() {
                        break;
                    }
                }
            }
            render_pipeline::pipeline(&mut out, &pipeline);
        }
        modular_groups::output(&mut out, &image);
        for a in colors {
            print_color(&mut out, &a.map(|v| v.get()));
        }
        return out;
    }
    let offset = number(args);
    let owner = number::<u32>(args);
    let plain = color(args);
    let cells = plain.map(Cell::new);
    let mut stage = if owner == 0 {
        get_spot_color_stage(offset, &plain)
    } else {
        get_spot_color_stage_cells(offset, &cells)
    };
    let current = || {
        if owner == 0 {
            plain
        } else {
            std::array::from_fn(|i| cells[i].get())
        }
    };
    let mut out = String::new();
    if kind == "spot-query" {
        let s = stage.settings();
        let prepared = stage.prepare_for_threads(3);
        write!(
            out,
            "{} {} {} {} {} {} {}",
            stage.get_name(),
            s.border_x,
            s.border_y,
            s.shift_x,
            s.shift_y,
            status_code(&stage.is_initialized()),
            status_code(&prepared)
        )
        .unwrap();
        let count = number::<usize>(args);
        for _ in 0..count {
            let c = number::<usize>(args);
            write!(out, " {}", stage.get_channel_mode(c) as i32).unwrap();
        }
        print_color(&mut out, &current());
        return out;
    }
    if kind == "spot-guard" {
        let left = number(args);
        let right = number(args);
        let s = stage.process_row(
            &Vec::new(),
            &Vec::new(),
            &mut RowBuffers {
                input: &mut [],
                output: &mut [],
            },
            left,
            right,
            23,
            91,
            73,
            17,
        );
        write!(out, "{}", status_code(&s)).unwrap();
        print_color(&mut out, &current());
        return out;
    }
    if kind == "spot-sample" {
        let p = number::<u32>(args);
        let s = number::<u32>(args);
        let mut planes: Vec<_> = (0..4)
            .map(|_| Plane::<f32>::create(OFFSET + 1, 1).unwrap())
            .collect();
        for (c, plane) in planes.iter_mut().enumerate() {
            plane.row_mut(0)[OFFSET] = f32::from_bits(if c == 3 { s } else { p });
        }
        let rows: RowInfo = (0..4)
            .map(|c| {
                vec![Row {
                    storage: RowStorage::Input,
                    channel: c,
                    y: 0,
                    x: 0,
                }]
            })
            .collect();
        let status = stage.process_row(
            &rows,
            &Vec::new(),
            &mut RowBuffers {
                input: &mut planes,
                output: &mut [],
            },
            0,
            0,
            1,
            0,
            0,
            0,
        );
        write!(out, "{}", status_code(&status)).unwrap();
        for p in planes {
            write!(out, " {}", p.row(0)[OFFSET].to_bits()).unwrap();
        }
        print_color(&mut out, &current());
        return out;
    }
    let width = number::<usize>(args);
    let left = number(args);
    let right = number(args);
    let origin = number::<usize>(args);
    let passes = number::<usize>(args);
    let mode = number(args);
    let len = origin + OFFSET + width + 9;
    let mut planes: Vec<_> = (0..7)
        .map(|_| Plane::<f32>::create(len, 3).unwrap())
        .collect();
    for (c, p) in planes.iter_mut().enumerate() {
        for y in 0..3 {
            for x in 0..len {
                p.row_mut(y)[x] = render_pipeline::value(c, x, y, 0, mode);
            }
        }
    }
    let rows: RowInfo = (0..7)
        .map(|c| {
            vec![Row {
                storage: RowStorage::Input,
                channel: c,
                y: 1,
                x: origin,
            }]
        })
        .collect();
    for _ in 0..passes {
        let changed = color(args);
        if owner != 0 {
            for c in 0..4 {
                cells[c].set(changed[c]);
            }
        }
        let s = stage.process_row(
            &rows,
            &Vec::new(),
            &mut RowBuffers {
                input: &mut planes,
                output: &mut [],
            },
            left,
            right,
            width,
            97,
            131,
            19,
        );
        write!(out, " {}", status_code(&s)).unwrap();
        for p in &planes {
            for y in 0..3 {
                for x in 0..len {
                    write!(out, " {}", p.row(y)[x].to_bits()).unwrap();
                }
            }
        }
        print_color(&mut out, &current());
    }
    out
}
fn words(a: [u32; 4]) -> String {
    a.map(|v| v.to_string()).join(" ")
}
fn exact(cases: Vec<String>) {
    for chunk in cases.chunks(256) {
        compare(chunk.to_vec());
    }
}
const COLOR: [u32; 4] = [0x3f000000, 0x3e4ccccd, 0x3f800000, 0x3f000000];
#[test]
fn spot_factory_modes_settings_wrapping_and_guard_before_access() {
    let mut q = Vec::new();
    for offset in [
        0,
        1,
        3,
        usize::MAX,
        usize::MAX - 1,
        usize::MAX - 2,
        usize::MAX - 3,
        usize::MAX - 4,
    ] {
        for owner in 0..2 {
            q.push(format!(
                "spot-query {offset} {owner} {} 10 0 1 2 3 4 5 6 7 {} {}",
                words(COLOR),
                offset.wrapping_add(3),
                usize::MAX
            ));
            for (l, r) in [(1, 0), (0, 1), (1, 1), (usize::MAX, usize::MAX)] {
                q.push(format!(
                    "spot-guard {offset} {owner} {} {l} {r}",
                    words([0x7fa12345, 0xffa54321, 0x80000000, 0x7fc12345])
                ));
            }
        }
    }
    exact(q);
}
#[test]
fn spot_all_four_operand_special_words_and_nan_priorities() {
    let p = [
        0, 0x80000000, 0x3f800000, 0xbf800000, 0x3f000000, 0x40000000, 0x7fc12345, 0xffc23456,
        0x7fa12345, 0xffa23456, 0x7f800000, 0xff800000, 1, 0x80000001, 0x3f7fffff, 0x3f800001,
    ];
    let mut q = Vec::new();
    for a in p {
        for c in p {
            for s in p {
                for bg in p {
                    q.push(format!("spot-sample 0 0 {} {bg} {s}", words([c, c, c, a])));
                }
            }
        }
    }
    exact(q);
}
#[test]
fn spot_finite_rounding_subnormals_and_random_operand_words() {
    let p = [
        1, 0x80000001, 0x007fffff, 0x807fffff, 0x00800000, 0x80800000, 0x7f7fffff, 0xff7fffff,
        0x3eaaaaab, 0xbeaaaaab, 0x3f800001, 0xbf7fffff,
    ];
    let mut q = Vec::new();
    for a in p {
        for c in p {
            for s in p {
                for bg in p {
                    q.push(format!("spot-sample 0 1 {} {bg} {s}", words([c, c, c, a])));
                }
            }
        }
    }
    let mut seed = 17;
    for _ in 0..4096 {
        let a = std::array::from_fn(|_| next(&mut seed) as u32);
        let p = next(&mut seed) as u32;
        let s = next(&mut seed) as u32;
        q.push(format!("spot-sample 0 0 {} {p} {s}", words(a)));
    }
    exact(q);
}
#[test]
fn spot_rows_preserve_padding_ignored_channels_and_live_coefficients() {
    let mut q = Vec::new();
    for owner in 0..2 {
        for offset in [0, 1, 3] {
            for (width, left, right, origin) in [
                (0, 0, 0, 0),
                (0, 0, 0, 7),
                (1, 0, 0, 0),
                (17, 0, 0, 3),
                (65, 0, 0, 1),
                (9, 1, 0, 2),
                (9, 0, 2, 3),
            ] {
                for mode in 0..4 {
                    for color in [
                        COLOR,
                        [0, 0x80000000, 0x3f800000, 0x40000000],
                        [0x7fa12345, 0xffa54321, 0x7f800000, 0xffc12345],
                    ] {
                        q.push(format!("spot-row {offset} {owner} {} {width} {left} {right} {origin} 3 {mode} {} {} {}",words(color),words(COLOR),words(color),words([0x3f800000,0xbf800000,0x3eaaaaab,0x80000000])));
                    }
                }
            }
        }
    }
    exact(q);
}
#[test]
fn spot_real_multigroup_simple_pipeline_and_ordered_spot_sequence() {
    let mut q = Vec::new();
    for (w, h) in [(1, 1), (17, 9), (257, 3), (3, 257), (513, 1)] {
        for count in 1..=3 {
            for reverse in 0..2 {
                for mode in 0..4 {
                    let mut s = format!("spot-simple {w} {h} {count} 2 {mode} {reverse}");
                    for i in 0..count {
                        write!(
                            s,
                            " {} {} {}",
                            [0, 2, 1][i],
                            words(COLOR),
                            words(if i % 2 == 0 {
                                COLOR
                            } else {
                                [0x3eaaaaab, 0x3f400000, 0x3f000000, 0x3f19999a]
                            })
                        )
                        .unwrap();
                    }
                    q.push(s);
                }
            }
        }
    }
    exact(q);
}
