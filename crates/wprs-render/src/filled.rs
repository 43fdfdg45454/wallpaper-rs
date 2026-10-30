use wgpu::BufferUsages;

pub(crate) fn filled(
    (device, queue): (&wgpu::Device, &wgpu::Queue),
    label: &str,
    contents: &[u8],
    usage: BufferUsages,
) -> wgpu::Buffer {
    let size = (contents.len() as u64)
        .next_multiple_of(wgpu::COPY_BUFFER_ALIGNMENT)
        .max(wgpu::COPY_BUFFER_ALIGNMENT);
    let usage = usage | BufferUsages::COPY_DST;
    let buffer = device.create_buffer(&wgpu::BufferDescriptor {
        label: Some(label),
        size,
        usage,
        mapped_at_creation: false,
    });
    let mut padded = contents.to_vec();
    padded.resize(size as usize, 0);
    queue.write_buffer(&buffer, 0, &padded);
    buffer
}
