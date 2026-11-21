
export async function create_video_writer(width, height) {
    const { createVideoWriter } = await import('/assets/static/js/replay_video.js');
    return createVideoWriter(width, height);
}
