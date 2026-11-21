// Encodes replay frames into a WebM video with the browser's WebCodecs encoder.
//
// Loaded only once a replay is first rendered to video, so it costs the game nothing before.
// The video has a frame every server tick, so players can seek anywhere, but the board only
// changes when a move lands or a cooldown ring charges up a step. Only those frames are
// encoded; with VP9 every other one is a one-byte frame telling the decoder to show the last
// picture again.

/**
 * Codecs tried in order, with the Matroska id of each and whether its frames can be repeated
 * without encoding them again.
 */
const CODECS = [
    { config: "vp09.00.10.08", matroska: "V_VP9", repeats: true },
    { config: "vp8", matroska: "V_VP8", repeats: false },
];

/** Longest stretch between keyframes, in milliseconds, so the video seeks quickly. */
const KEYFRAME_SPAN_MS = 30000;

/** Longest span of a cluster, in milliseconds, which its 16-bit relative timestamps allow. */
const CLUSTER_SPAN_MS = 32000;

/** Encoded frames allowed to wait for the encoder before the next one is held back. */
const MAX_QUEUED_FRAMES = 4;

/** How often a full encoder queue is checked again, in milliseconds. */
const QUEUE_POLL_MS = 5;

/**
 * Bitrate budget. Encoded frames are far apart, so every one gets plenty to stay sharp,
 * while frames that change a few squares come out small whatever the budget.
 */
const BITRATE = 8_000_000;

/** Returns a video writer for frames of `width` by `height` pixels. */
export async function createVideoWriter(width, height) {
    if (typeof VideoEncoder === "undefined") {
        throw new Error("This browser cannot encode video");
    }
    for (const codec of CODECS) {
        const config = {
            codec: codec.config,
            width,
            height,
            bitrate: BITRATE,
            framerate: 1,
            latencyMode: "quality",
        };
        const { supported } = await VideoEncoder.isConfigSupported(config);
        if (supported) {
            return new VideoWriter(config, codec);
        }
    }
    throw new Error("This browser cannot encode WebM video");
}

/** Collects the frames of a video and muxes them into a WebM file once finished. */
class VideoWriter {
    constructor(config, codec) {
        this.config = config;
        this.codec = codec;
        /** Every frame in order: encoded ones, and repeats of the frame before. */
        this.frames = [];
        /** Encoded frames by the millisecond they appear at. */
        this.encoded = new Map();
        this.error = null;
        this.lastKeyMs = -Infinity;
        this.encoder = new VideoEncoder({
            output: (chunk) => {
                const data = new Uint8Array(chunk.byteLength);
                chunk.copyTo(data);
                this.encoded.set(Math.round(chunk.timestamp / 1000), {
                    key: chunk.type === "key",
                    data,
                });
            },
            error: (error) => {
                this.error = error;
            },
        });
        this.encoder.configure(config);
    }

    /**
     * Adds the frame appearing `ms` milliseconds into the video: what `canvas` shows when
     * `changed`, otherwise a repeat of the frame before.
     */
    async addFrame(canvas, ms, changed) {
        this.check();
        if (!changed && this.codec.repeats && this.frames.length > 0) {
            this.frames.push({ ms, repeat: true });
            return;
        }
        while (this.encoder.encodeQueueSize > MAX_QUEUED_FRAMES) {
            await new Promise((resolve) => setTimeout(resolve, QUEUE_POLL_MS));
        }
        const keyFrame = ms - this.lastKeyMs >= KEYFRAME_SPAN_MS;
        if (keyFrame) {
            this.lastKeyMs = ms;
        }
        const frame = new VideoFrame(canvas, { timestamp: Math.round(ms * 1000) });
        this.encoder.encode(frame, { keyFrame });
        frame.close();
        this.frames.push({ ms, repeat: false });
    }

    /** Finishes a video lasting `durationMs` milliseconds and returns it as a file. */
    async finish(durationMs) {
        await this.encoder.flush();
        this.encoder.close();
        this.check();
        return new Blob(muxWebm(this.chunks(), this.config, this.codec.matroska, durationMs), {
            type: "video/webm",
        });
    }

    /**
     * Returns the data of every frame in order. A repeat shows again the reference slot
     * holding the picture shown last; a frame the encoder dropped, or a repeat with no slot
     * holding that picture, is left out, so the picture before stays a little longer.
     */
    chunks() {
        const chunks = [];
        const slots = new Vp9Slots();
        for (const frame of this.frames) {
            if (frame.repeat) {
                const data = slots.showLast();
                if (data) {
                    chunks.push({ ms: frame.ms, key: false, data });
                }
                continue;
            }
            const encoded = this.encoded.get(Math.round(frame.ms));
            if (encoded) {
                if (this.codec.repeats) {
                    slots.decode(encoded.data);
                }
                chunks.push({ ms: frame.ms, ...encoded });
            }
        }
        return chunks;
    }

    /** Throws the error the encoder ran into, if any. */
    check() {
        if (this.error) {
            throw this.error;
        }
    }
}

/**
 * Follows which of the eight VP9 reference slots hold the picture shown last, by reading the
 * headers of the encoded frames.
 */
class Vp9Slots {
    constructor() {
        /** Bit mask of the slots holding the picture shown last. */
        this.shown = 0;
        this.profile = 0;
    }

    /** Reads the frames of one encoded chunk, which may hold several in a superframe. */
    decode(data) {
        for (const frame of superframeParts(data)) {
            const header = readHeader(frame);
            if (!header) {
                this.shown = 0;
                continue;
            }
            this.profile = header.profile;
            if (header.showExisting !== undefined) {
                this.shown = 1 << header.showExisting;
            } else if (header.show) {
                this.shown = header.refresh;
            } else {
                this.shown &= ~header.refresh;
            }
        }
    }

    /** Returns a frame showing the picture shown last again, or null when no slot holds it. */
    showLast() {
        if (this.shown === 0 || this.profile > 2) {
            return null;
        }
        const slot = 31 - Math.clz32(this.shown & -this.shown);
        // Frame marker 0b10, the two profile bits, show_existing_frame, then the slot.
        const profileBits = ((this.profile & 1) << 5) | ((this.profile >> 1) << 4);
        return Uint8Array.of(0x80 | profileBits | 0x08 | slot);
    }
}

/** Returns the frames of a VP9 chunk, split along its superframe index if it has one. */
function superframeParts(data) {
    const marker = data[data.length - 1];
    if ((marker & 0xe0) !== 0xc0) {
        return [data];
    }
    const count = (marker & 0x7) + 1;
    const sizeBytes = ((marker >> 3) & 0x3) + 1;
    const indexSize = 2 + sizeBytes * count;
    if (data.length < indexSize || data[data.length - indexSize] !== marker) {
        return [data];
    }
    const parts = [];
    let offset = 0;
    let at = data.length - indexSize + 1;
    for (let index = 0; index < count; index++) {
        let size = 0;
        for (let byte = 0; byte < sizeBytes; byte++) {
            size |= data[at++] << (8 * byte);
        }
        parts.push(data.subarray(offset, offset + size));
        offset += size;
    }
    return parts;
}

/**
 * Reads what a VP9 frame header says about the reference slots: whether the frame is shown
 * and which slots it refreshes, or which slot it shows again. Returns null for a header it
 * cannot read.
 */
function readHeader(frame) {
    let bit = 0;
    const read = (count) => {
        let value = 0;
        for (let index = 0; index < count; index++, bit++) {
            if (bit >> 3 >= frame.length) {
                throw new RangeError("Truncated VP9 header");
            }
            value = (value << 1) | ((frame[bit >> 3] >> (7 - (bit & 7))) & 1);
        }
        return value;
    };
    try {
        if (read(2) !== 2) {
            return null;
        }
        const low = read(1);
        const profile = (read(1) << 1) | low;
        if (profile === 3) {
            read(1);
        }
        if (read(1)) {
            return { profile, showExisting: read(3) };
        }
        const keyFrame = read(1) === 0;
        const show = read(1) === 1;
        const errorResilient = read(1) === 1;
        if (keyFrame) {
            return { profile, show, refresh: 0xff };
        }
        const intraOnly = show ? false : read(1) === 1;
        if (!errorResilient) {
            read(2);
        }
        if (intraOnly) {
            if (profile > 0) {
                // Intra-only frames of higher profiles carry a colour config first.
                return null;
            }
            read(24);
        }
        return { profile, show, refresh: read(8) };
    } catch {
        return null;
    }
}

/** Matroska element ids. */
const ID = Object.freeze({
    EBML: 0x1a45dfa3,
    EBML_VERSION: 0x4286,
    EBML_READ_VERSION: 0x42f7,
    EBML_MAX_ID_LENGTH: 0x42f2,
    EBML_MAX_SIZE_LENGTH: 0x42f3,
    DOC_TYPE: 0x4282,
    DOC_TYPE_VERSION: 0x4287,
    DOC_TYPE_READ_VERSION: 0x4285,
    SEGMENT: 0x18538067,
    SEEK_HEAD: 0x114d9b74,
    SEEK: 0x4dbb,
    SEEK_ID: 0x53ab,
    SEEK_POSITION: 0x53ac,
    INFO: 0x1549a966,
    TIMESTAMP_SCALE: 0x2ad7b1,
    MUXING_APP: 0x4d80,
    WRITING_APP: 0x5741,
    DURATION: 0x4489,
    TRACKS: 0x1654ae6b,
    TRACK_ENTRY: 0xae,
    TRACK_NUMBER: 0xd7,
    TRACK_UID: 0x73c5,
    TRACK_TYPE: 0x83,
    CODEC_ID: 0x86,
    VIDEO: 0xe0,
    PIXEL_WIDTH: 0xb0,
    PIXEL_HEIGHT: 0xba,
    CUES: 0x1c53bb6b,
    CUE_POINT: 0xbb,
    CUE_TIME: 0xb3,
    CUE_TRACK_POSITIONS: 0xb7,
    CUE_TRACK: 0xf7,
    CUE_CLUSTER_POSITION: 0xf1,
    CLUSTER: 0x1f43b675,
    CLUSTER_TIMESTAMP: 0xe7,
    SIMPLE_BLOCK: 0xa3,
});

/** The only track of the video. */
const TRACK = 1;

/** Matroska track type of a video track. */
const VIDEO_TRACK_TYPE = 1;

/**
 * Builds the WebM file of `chunks`: the header, an index of where the top-level elements
 * start, the index of keyframes, then the frames grouped in clusters. A cluster starts at
 * every keyframe, which the index points to, and wherever the one before grew too long.
 */
function muxWebm(chunks, config, codecId, durationMs) {
    const clusters = [];
    for (const chunk of chunks) {
        const last = clusters[clusters.length - 1];
        if (!last || chunk.key || chunk.ms - last.ms >= CLUSTER_SPAN_MS) {
            clusters.push({ ms: chunk.ms, key: chunk.key, blocks: [] });
        }
        const cluster = clusters[clusters.length - 1];
        cluster.blocks.push(simpleBlock(chunk, chunk.ms - cluster.ms));
    }
    const cued = clusters.filter((cluster) => cluster.key);
    const clusterBytes = clusters.map((cluster) =>
        element(ID.CLUSTER, [uintElement(ID.CLUSTER_TIMESTAMP, cluster.ms), ...cluster.blocks]),
    );

    const info = element(ID.INFO, [
        uintElement(ID.TIMESTAMP_SCALE, 1_000_000), // Timestamps count milliseconds.
        stringElement(ID.MUXING_APP, "ffchess"),
        stringElement(ID.WRITING_APP, "ffchess"),
        floatElement(ID.DURATION, durationMs),
    ]);
    const tracks = element(ID.TRACKS, [
        element(ID.TRACK_ENTRY, [
            uintElement(ID.TRACK_NUMBER, TRACK),
            uintElement(ID.TRACK_UID, TRACK),
            uintElement(ID.TRACK_TYPE, VIDEO_TRACK_TYPE),
            stringElement(ID.CODEC_ID, codecId),
            element(ID.VIDEO, [
                uintElement(ID.PIXEL_WIDTH, config.width),
                uintElement(ID.PIXEL_HEIGHT, config.height),
            ]),
        ]),
    ]);

    // The positions in the indexes are written at a fixed width, so the indexes have the same
    // size whatever they hold, and every offset can be worked out before they are written.
    const cuesFor = (positions) =>
        element(
            ID.CUES,
            cued.map((cluster) =>
                element(ID.CUE_POINT, [
                    uintElement(ID.CUE_TIME, cluster.ms, 8),
                    element(ID.CUE_TRACK_POSITIONS, [
                        uintElement(ID.CUE_TRACK, TRACK),
                        uintElement(ID.CUE_CLUSTER_POSITION, positions.get(cluster) ?? 0, 8),
                    ]),
                ]),
            ),
        );
    const seekHeadFor = (positions) =>
        element(
            ID.SEEK_HEAD,
            [ID.INFO, ID.TRACKS, ID.CUES].map((id, index) =>
                element(ID.SEEK, [
                    element(ID.SEEK_ID, [idBytes(id)]),
                    uintElement(ID.SEEK_POSITION, positions[index], 8),
                ]),
            ),
        );
    const seekHeadSize = seekHeadFor([0, 0, 0]).length;
    const cuesAt = seekHeadSize + info.length + tracks.length;
    const headerSize = cuesAt + cuesFor(new Map()).length;
    const positions = new Map();
    let offset = headerSize;
    clusters.forEach((cluster, index) => {
        positions.set(cluster, offset);
        offset += clusterBytes[index].length;
    });
    const cues = cuesFor(positions);
    const seekHead = seekHeadFor([seekHeadSize, seekHeadSize + info.length, cuesAt]);

    const ebmlHeader = element(ID.EBML, [
        uintElement(ID.EBML_VERSION, 1),
        uintElement(ID.EBML_READ_VERSION, 1),
        uintElement(ID.EBML_MAX_ID_LENGTH, 4),
        uintElement(ID.EBML_MAX_SIZE_LENGTH, 8),
        stringElement(ID.DOC_TYPE, "webm"),
        uintElement(ID.DOC_TYPE_VERSION, 4),
        uintElement(ID.DOC_TYPE_READ_VERSION, 2),
    ]);
    return [
        ebmlHeader,
        elementHeader(ID.SEGMENT, offset),
        seekHead,
        info,
        tracks,
        cues,
        ...clusterBytes,
    ];
}

/** Returns the block of one frame of the track, `relativeMs` into its cluster. */
function simpleBlock(chunk, relativeMs) {
    const header = new Uint8Array(4);
    header[0] = 0x80 | TRACK; // The track number as a one-byte variable-length integer.
    new DataView(header.buffer).setInt16(1, relativeMs);
    header[3] = chunk.key ? 0x80 : 0;
    return element(ID.SIMPLE_BLOCK, [header, chunk.data]);
}

/** Returns an element holding the concatenation of `children`. */
function element(id, children) {
    const size = children.reduce((sum, child) => sum + child.length, 0);
    return concat([elementHeader(id, size), ...children]);
}

/** Returns the id and size of an element holding `size` bytes. */
function elementHeader(id, size) {
    return concat([idBytes(id), sizeBytes(size)]);
}

/** Returns an element holding an unsigned integer, in `width` bytes or as few as it needs. */
function uintElement(id, value, width) {
    const bytes = [];
    let rest = BigInt(value);
    do {
        bytes.unshift(Number(rest & 0xffn));
        rest >>= 8n;
    } while (rest > 0n);
    while (width && bytes.length < width) {
        bytes.unshift(0);
    }
    return element(id, [Uint8Array.from(bytes)]);
}

/** Returns an element holding a 64-bit float. */
function floatElement(id, value) {
    const bytes = new Uint8Array(8);
    new DataView(bytes.buffer).setFloat64(0, value);
    return element(id, [bytes]);
}

/** Returns an element holding an ASCII string. */
function stringElement(id, value) {
    return element(id, [new TextEncoder().encode(value)]);
}

/** Returns the bytes of an element id, which carries its own length marker. */
function idBytes(id) {
    const bytes = [];
    for (let rest = id; rest > 0; rest = Math.floor(rest / 256)) {
        bytes.unshift(rest & 0xff);
    }
    return Uint8Array.from(bytes);
}

/**
 * Returns `size` as a variable-length integer, as short as it fits in. A length whose bits are
 * all set means an unknown size, so it is never used for a known one.
 */
function sizeBytes(size) {
    let length = 1;
    while (BigInt(size) >= (1n << BigInt(7 * length)) - 1n) {
        length++;
    }
    const bytes = new Uint8Array(length);
    let rest = BigInt(size);
    for (let index = length - 1; index >= 0; index--) {
        bytes[index] = Number(rest & 0xffn);
        rest >>= 8n;
    }
    bytes[0] |= 0x80 >> (length - 1);
    return bytes;
}

/** Joins byte arrays. */
function concat(parts) {
    const out = new Uint8Array(parts.reduce((sum, part) => sum + part.length, 0));
    let offset = 0;
    for (const part of parts) {
        out.set(part, offset);
        offset += part.length;
    }
    return out;
}
