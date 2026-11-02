// Copyright (c) the JPEG XL Project Authors. All rights reserved.
// Use of this source code is governed by a BSD-style license in LICENSE.
use jxl::jxl::base::status::StatusCode;
use jxl::jxl::image::Plane;
use jxl::jxl::render_pipeline::{
    render_pipeline_stage::{
        RENDER_PIPELINE_X_OFFSET as OFFSET, RenderPipelineChannelMode as Mode, Row, RowBuffers,
        RowInfo, RowStorage, Settings,
    },
    stage_spot::{get_spot_color_stage, get_spot_color_stage_cells},
};
use std::cell::Cell;
fn rows() -> RowInfo {
    (0..5)
        .map(|c| {
            vec![Row {
                storage: RowStorage::Input,
                channel: c,
                y: 0,
                x: 3,
            }]
        })
        .collect()
}
#[test]
fn spot_original_dyadic_samples_and_ignored_storage_are_retained() {
    let color = [0.5, 0.25, 1., 0.5];
    let mut stage = get_spot_color_stage(1, &color);
    let mut planes: Vec<_> = (0..5)
        .map(|_| Plane::<f32>::create(OFFSET + 9, 1).unwrap())
        .collect();
    for p in &mut planes {
        p.row_mut(0)[..OFFSET + 9].fill(f32::from_bits(0x7fa12345));
        p.row_mut(0)[OFFSET + 3] = 0.25;
    }
    planes[4].row_mut(0)[OFFSET + 3] = 0.5;
    let before: Vec<Vec<_>> = planes
        .iter()
        .map(|p| p.row(0)[..OFFSET + 9].iter().map(|v| v.to_bits()).collect())
        .collect();
    stage
        .process_row(
            &rows(),
            &vec![],
            &mut RowBuffers {
                input: &mut planes,
                output: &mut [],
            },
            0,
            0,
            1,
            99,
            77,
            5,
        )
        .unwrap();
    // Captured independently from the pinned scalar factory: 0.3125, 0.25, 0.4375.
    for (c, expected) in [0x3ea00000, 0x3e800000, 0x3ee00000].into_iter().enumerate() {
        assert_eq!(planes[c].row(0)[OFFSET + 3].to_bits(), expected);
    }
    for (c, original) in before.iter().enumerate() {
        for (x, word) in original.iter().enumerate() {
            if c >= 3 || x != OFFSET + 3 {
                assert_eq!(planes[c].row(0)[x].to_bits(), *word);
            }
        }
    }
    assert_eq!(stage.settings(), Settings::default());
    assert_eq!(stage.get_name(), "Spot");
    assert_eq!(stage.get_channel_mode(3), Mode::Ignored);
    assert_eq!(stage.get_channel_mode(4), Mode::Input);
}
#[test]
fn spot_external_coefficients_remain_live_between_calls() {
    let color = [0.5, 0.25, 1., 0.5].map(Cell::new);
    let mut stage = get_spot_color_stage_cells(1, &color);
    let mut planes: Vec<_> = (0..5)
        .map(|_| Plane::<f32>::create(OFFSET + 9, 1).unwrap())
        .collect();
    for p in &mut planes {
        p.row_mut(0)[OFFSET + 3] = 0.25;
    }
    planes[4].row_mut(0)[OFFSET + 3] = 0.5;
    stage
        .process_row(
            &rows(),
            &vec![],
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
        )
        .unwrap();
    for (c, v) in [1., 0.5, 0., 2.].into_iter().enumerate() {
        color[c].set(v);
    }
    stage
        .process_row(
            &rows(),
            &vec![],
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
        )
        .unwrap();
    for (c, v) in [1., 0.5, 0.].into_iter().enumerate() {
        assert_eq!(planes[c].row(0)[OFFSET + 3].to_bits(), f32::to_bits(v));
    }
    assert_eq!(color[3].get(), 2.);
}
#[test]
fn spot_guard_precedes_row_access_and_factory_offset_wraps() {
    let color = [f32::from_bits(0x7fa12345); 4];
    for offset in [0, usize::MAX, usize::MAX - 1, usize::MAX - 2] {
        let mut stage = get_spot_color_stage(offset, &color);
        for (l, r) in [(1, 0), (0, 1), (usize::MAX, usize::MAX)] {
            assert_eq!(
                stage.process_row(
                    &vec![],
                    &vec![],
                    &mut RowBuffers {
                        input: &mut [],
                        output: &mut []
                    },
                    l,
                    r,
                    1,
                    0,
                    0,
                    0
                ),
                Err(StatusCode::GenericError)
            );
        }
        for c in 0..6 {
            assert_eq!(
                stage.get_channel_mode(c),
                if c < 3 {
                    Mode::InPlace
                } else if c == offset.wrapping_add(3) {
                    Mode::Input
                } else {
                    Mode::Ignored
                }
            );
        }
    }
}
#[test]
fn spot_empty_span_preserves_raw_signaling_words_with_valid_rows() {
    let color = [f32::from_bits(0xffa12345); 4];
    let mut stage = get_spot_color_stage(1, &color);
    let mut planes: Vec<_> = (0..5)
        .map(|_| Plane::<f32>::create(OFFSET + 9, 1).unwrap())
        .collect();
    for p in &mut planes {
        p.row_mut(0)[..OFFSET + 9].fill(f32::from_bits(0x7fa54321));
    }
    stage
        .process_row(
            &rows(),
            &vec![],
            &mut RowBuffers {
                input: &mut planes,
                output: &mut [],
            },
            0,
            0,
            0,
            0,
            0,
            0,
        )
        .unwrap();
    for p in planes {
        assert!(
            p.row(0)[..OFFSET + 9]
                .iter()
                .all(|v| v.to_bits() == 0x7fa54321)
        );
    }
    assert!(color.iter().all(|v| v.to_bits() == 0xffa12345));
}
