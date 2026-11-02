// Copyright (c) the JPEG XL Project Authors. All rights reserved.
// Use of this source code is governed by a BSD-style license in LICENSE.
//! Complete scalar SpotColorStage/factory from render_pipeline/stage_spot.{h,cc}.
//! Coefficients retain their external array lifetime rather than a snapshot.
use super::render_pipeline_stage::{
    RenderPipelineChannelMode as Mode, RenderPipelineStage, RowBuffers, RowInfo, Settings,
};
#[cfg(not(all(target_os = "linux", target_arch = "x86_64")))]
use crate::jxl::base::float::mul_add_f32;
use crate::jxl::base::status::{Status, StatusCode};
use std::cell::Cell;

// The selected pristine Linux x86-64 stage evaluates each scalar operation
// separately. Keep that operand order across Rust optimization, including
// competing NaNs, without changing the source's AArch64 contraction site.
#[cfg(all(target_os = "linux", target_arch = "x86_64"))]
#[inline(never)]
fn mul(a: f32, b: f32) -> f32 {
    a * b
}
#[cfg(all(target_os = "linux", target_arch = "x86_64"))]
#[inline(never)]
fn add(a: f32, b: f32) -> f32 {
    a + b
}

enum Color<'a> {
    Borrowed(&'a [f32; 4]),
    Cells(&'a [Cell<f32>; 4]),
}
impl Color<'_> {
    fn get(&self, c: usize) -> f32 {
        match self {
            Self::Borrowed(color) => color[c],
            Self::Cells(color) => color[c].get(),
        }
    }
}
struct SpotColorStage<'a> {
    spot_c: usize,
    spot_color: Color<'a>,
}
impl RenderPipelineStage for SpotColorStage<'_> {
    fn settings(&self) -> Settings {
        Settings::default()
    }
    fn get_channel_mode(&self, c: usize) -> Mode {
        if c < 3 {
            Mode::InPlace
        } else if c == self.spot_c {
            Mode::Input
        } else {
            Mode::Ignored
        }
    }
    fn get_name(&self) -> &str {
        "Spot"
    }
    fn process_row(
        &mut self,
        input: &RowInfo,
        _output: &RowInfo,
        buffers: &mut RowBuffers<'_>,
        left: usize,
        right: usize,
        width: usize,
        _x: usize,
        _y: usize,
        _thread: usize,
    ) -> Status {
        if left != 0 || right != 0 {
            return Err(StatusCode::GenericError);
        }
        let scale = self.spot_color.get(3);
        for c in 0..3 {
            let p = self.get_input_row(input, c, 0);
            let s = self.get_input_row(input, self.spot_c, 0);
            for x in 0..width {
                #[cfg(all(target_os = "linux", target_arch = "x86_64"))]
                let value = {
                    let mix = mul(scale, buffers.row(s)[x]);
                    add(
                        mul(mix, self.spot_color.get(c)),
                        mul(1.0 - mix, buffers.row(p)[x]),
                    )
                };
                #[cfg(not(all(target_os = "linux", target_arch = "x86_64")))]
                let value = {
                    let mix = scale * buffers.row(s)[x];
                    mul_add_f32(mix, self.spot_color.get(c), (1.0 - mix) * buffers.row(p)[x])
                };
                buffers.row_mut(p)[x] = value;
            }
        }
        Ok(())
    }
}
pub fn get_spot_color_stage(
    spot_c_offset: usize,
    spot_color: &[f32; 4],
) -> Box<dyn RenderPipelineStage + '_> {
    Box::new(SpotColorStage {
        spot_c: spot_c_offset.wrapping_add(3),
        spot_color: Color::Borrowed(spot_color),
    })
}
/// The same borrowed pointee semantics with safe mutations between stage calls.
pub fn get_spot_color_stage_cells(
    spot_c_offset: usize,
    spot_color: &[Cell<f32>; 4],
) -> Box<dyn RenderPipelineStage + '_> {
    Box::new(SpotColorStage {
        spot_c: spot_c_offset.wrapping_add(3),
        spot_color: Color::Cells(spot_color),
    })
}
