# Voidin

<img src="./assets/clapping.gif" width=550 height=480>

## Overview

The goal of this project is to research and implement modern techniques for rendering large scenes.  The code is intended to serve as a reference for others wanting to build their own renderers and template for my own explorations.

## Features

- GPU driven rendering for greatly reduced driver overhead
- Compute based frustum culling
- Instancing for reduced memory usage
- Deferred renderer artitecture
- LTC based arealights
- TAA

## Building

Run `cargo build --release` from the main repository directory. Don't forget the --release since debug builds in Rust will run slowly.

## References

* [Real-Time Polygonal-Light Shading with Linearly Transformed Cosines](https://eheitzresearch.wordpress.com/415-2/) (2016)
* [World position reconstruction from depth](https://github.com/ARM-software/vulkan_best_practice_for_mobile_developers/blob/master/samples/performance/render_subpasses/render_subpasses_tutorial.md)
* [Two-Pass Occlusion Culling](https://interplayoflight.wordpress.com/2017/11/15/experiments-in-gpu-based-occlusion-culling/)
