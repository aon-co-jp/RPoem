// 技術検証スパイク(2026-08-25): wasm32-unknown-unknown + wgpuのWebGPU
// バックエンドが、ブラウザ内で単純な計算(このスパイクでは1要素バッファへの
// 加算コンピュートシェーダー)を実行できるかどうかの最小実証。
// open-cuda/open-directx/aruaru-llmの本格移植ではなく、あくまで
// 「ブラウザのWebGPU経由でRust製コンピュートシェーダーが動くか」という
// 実現可能性の確認に限定する。
use wasm_bindgen::prelude::*;

#[wasm_bindgen(start)]
pub fn start() {
    console_error_panic_hook::set_once();
}

#[wasm_bindgen]
pub async fn run_add_one_spike(input: f32) -> Result<f32, JsValue> {
    let instance = wgpu::Instance::default();
    let adapter = instance
        .request_adapter(&wgpu::RequestAdapterOptions::default())
        .await
        .ok_or_else(|| JsValue::from_str("no WebGPU adapter available in this browser"))?;
    let (device, queue) = adapter
        .request_device(&wgpu::DeviceDescriptor::default(), None)
        .await
        .map_err(|e| JsValue::from_str(&format!("request_device failed: {e}")))?;

    let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
        label: Some("add_one"),
        source: wgpu::ShaderSource::Wgsl(
            r#"
            @group(0) @binding(0) var<storage, read_write> data: array<f32>;
            @compute @workgroup_size(1)
            fn main() {
                data[0] = data[0] + 1.0;
            }
            "#
            .into(),
        ),
    });

    let buf = device.create_buffer(&wgpu::BufferDescriptor {
        label: None,
        size: 4,
        usage: wgpu::BufferUsages::STORAGE
            | wgpu::BufferUsages::COPY_SRC
            | wgpu::BufferUsages::COPY_DST,
        mapped_at_creation: false,
    });
    queue.write_buffer(&buf, 0, bytemuck_cast(input));

    let bind_group_layout =
        device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: None,
            entries: &[wgpu::BindGroupLayoutEntry {
                binding: 0,
                visibility: wgpu::ShaderStages::COMPUTE,
                ty: wgpu::BindingType::Buffer {
                    ty: wgpu::BufferBindingType::Storage { read_only: false },
                    has_dynamic_offset: false,
                    min_binding_size: None,
                },
                count: None,
            }],
        });
    let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
        label: None,
        layout: &bind_group_layout,
        entries: &[wgpu::BindGroupEntry {
            binding: 0,
            resource: buf.as_entire_binding(),
        }],
    });
    let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
        label: None,
        bind_group_layouts: &[&bind_group_layout],
        push_constant_ranges: &[],
    });
    let pipeline = device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
        label: None,
        layout: Some(&pipeline_layout),
        module: &shader,
        entry_point: "main",
        compilation_options: Default::default(),
    });

    let readback = device.create_buffer(&wgpu::BufferDescriptor {
        label: None,
        size: 4,
        usage: wgpu::BufferUsages::MAP_READ | wgpu::BufferUsages::COPY_DST,
        mapped_at_creation: false,
    });

    let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor::default());
    {
        let mut pass = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor::default());
        pass.set_pipeline(&pipeline);
        pass.set_bind_group(0, &bind_group, &[]);
        pass.dispatch_workgroups(1, 1, 1);
    }
    encoder.copy_buffer_to_buffer(&buf, 0, &readback, 0, 4);
    queue.submit(Some(encoder.finish()));

    let slice = readback.slice(..);
    let (tx, rx) = futures_channel::oneshot::channel();
    slice.map_async(wgpu::MapMode::Read, move |res| {
        let _ = tx.send(res);
    });
    device.poll(wgpu::Maintain::Wait);
    rx.await
        .map_err(|_| JsValue::from_str("map_async channel dropped"))?
        .map_err(|e| JsValue::from_str(&format!("map_async failed: {e:?}")))?;
    let data = slice.get_mapped_range();
    let result = f32::from_le_bytes(data[0..4].try_into().unwrap());
    Ok(result)
}

fn bytemuck_cast(v: f32) -> &'static [u8] {
    // 簡易スパイクのため`bytemuck`は追加せず、Boxリークで済ませる
    // (本実装では避けるべき手抜きだが、検証目的のみのコードのため許容)
    Box::leak(Box::new(v.to_le_bytes()))
}
