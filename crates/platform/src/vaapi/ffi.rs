//! VA-API declarations (libva 1.x, `va/va.h` and `va/va_drm.h`), written by hand from the
//! MIT-licensed headers, and the run-time loader. Nothing links libva: `libva.so.2` and
//! `libva-drm.so.2` are opened with `dlopen` when hardware decoding is first tried, so FilmCraft
//! builds and runs on systems without them. Layouts are checked against the C compiler's view of the
//! headers in `abi_tests.rs`.
#![allow(non_camel_case_types, non_snake_case, non_upper_case_globals, dead_code)]

use std::ffi::{c_char, c_int, c_uint, c_void};

pub type VADisplay = *mut c_void;
pub type VAStatus = c_int;
pub type VAGenericID = c_uint;
pub type VAConfigID = VAGenericID;
pub type VAContextID = VAGenericID;
pub type VASurfaceID = VAGenericID;
pub type VABufferID = VAGenericID;
pub type VAImageID = VAGenericID;
pub type VAProfile = c_int;
pub type VAEntrypoint = c_int;
pub type VAConfigAttribType = c_int;
pub type VABufferType = c_int;

pub const VA_STATUS_SUCCESS: VAStatus = 0;
pub const VA_INVALID_ID: c_uint = 0xffff_ffff;
pub const VA_INVALID_SURFACE: VASurfaceID = VA_INVALID_ID;

pub const VAProfileH264Main: VAProfile = 6;
pub const VAProfileH264High: VAProfile = 7;
pub const VAProfileH264ConstrainedBaseline: VAProfile = 13;
pub const VAProfileHEVCMain: VAProfile = 17;
pub const VAProfileHEVCMain10: VAProfile = 18;

pub const VAEntrypointVLD: VAEntrypoint = 1;

pub const VAConfigAttribRTFormat: VAConfigAttribType = 0;
pub const VAConfigAttribMaxPictureWidth: VAConfigAttribType = 18;
pub const VAConfigAttribMaxPictureHeight: VAConfigAttribType = 19;
pub const VA_ATTRIB_NOT_SUPPORTED: u32 = 0x8000_0000;

pub const VA_RT_FORMAT_YUV420: u32 = 0x0000_0001;
pub const VA_RT_FORMAT_YUV420_10: u32 = 0x0000_0100;

pub const VA_PROGRESSIVE: c_int = 0x1;

pub const VAPictureParameterBufferType: VABufferType = 0;
pub const VAIQMatrixBufferType: VABufferType = 1;
pub const VASliceParameterBufferType: VABufferType = 4;
pub const VASliceDataBufferType: VABufferType = 5;

pub const VA_SLICE_DATA_FLAG_ALL: u32 = 0x00;

pub const VA_PICTURE_H264_INVALID: u32 = 0x01;
pub const VA_PICTURE_H264_TOP_FIELD: u32 = 0x02;
pub const VA_PICTURE_H264_BOTTOM_FIELD: u32 = 0x04;
pub const VA_PICTURE_H264_SHORT_TERM_REFERENCE: u32 = 0x08;
pub const VA_PICTURE_H264_LONG_TERM_REFERENCE: u32 = 0x10;

pub const VA_FOURCC_NV12: u32 = u32::from_le_bytes(*b"NV12");
pub const VA_FOURCC_P010: u32 = u32::from_le_bytes(*b"P010");

const VA_PADDING_LOW: usize = 4;
const VA_PADDING_MEDIUM: usize = 8;

#[repr(C)]
#[derive(Clone, Copy, Debug, Default)]
pub struct VAConfigAttrib {
    pub type_: VAConfigAttribType,
    pub value: u32,
}

#[repr(C)]
#[derive(Clone, Copy, Debug, Default)]
pub struct VAImageFormat {
    pub fourcc: u32,
    pub byte_order: u32,
    pub bits_per_pixel: u32,
    pub depth: u32,
    pub red_mask: u32,
    pub green_mask: u32,
    pub blue_mask: u32,
    pub alpha_mask: u32,
    pub va_reserved: [u32; VA_PADDING_LOW],
}

#[repr(C)]
#[derive(Clone, Copy, Debug, Default)]
pub struct VAImage {
    pub image_id: VAImageID,
    pub format: VAImageFormat,
    pub buf: VABufferID,
    pub width: u16,
    pub height: u16,
    pub data_size: u32,
    pub num_planes: u32,
    pub pitches: [u32; 3],
    pub offsets: [u32; 3],
    pub num_palette_entries: i32,
    pub entry_bytes: i32,
    pub component_order: [i8; 4],
    pub va_reserved: [u32; VA_PADDING_LOW],
}

// ------------------------------------------------------------------------------------- H.264

#[repr(C)]
#[derive(Clone, Copy, Debug, Default)]
pub struct VAPictureH264 {
    pub picture_id: VASurfaceID,
    pub frame_idx: u32,
    pub flags: u32,
    pub TopFieldOrderCnt: i32,
    pub BottomFieldOrderCnt: i32,
    pub va_reserved: [u32; VA_PADDING_LOW],
}

impl VAPictureH264 {
    pub const INVALID: VAPictureH264 = VAPictureH264 {
        picture_id: VA_INVALID_SURFACE,
        frame_idx: 0,
        flags: VA_PICTURE_H264_INVALID,
        TopFieldOrderCnt: 0,
        BottomFieldOrderCnt: 0,
        va_reserved: [0; VA_PADDING_LOW],
    };
}

#[repr(C)]
#[derive(Clone, Copy, Debug)]
pub struct VAPictureParameterBufferH264 {
    pub CurrPic: VAPictureH264,
    pub ReferenceFrames: [VAPictureH264; 16],
    pub picture_width_in_mbs_minus1: u16,
    pub picture_height_in_mbs_minus1: u16,
    pub bit_depth_luma_minus8: u8,
    pub bit_depth_chroma_minus8: u8,
    pub num_ref_frames: u8,
    /// `seq_fields` bit field (see [`h264_seq_fields`]).
    pub seq_fields: u32,
    pub num_slice_groups_minus1: u8,
    pub slice_group_map_type: u8,
    pub slice_group_change_rate_minus1: u16,
    pub pic_init_qp_minus26: i8,
    pub pic_init_qs_minus26: i8,
    pub chroma_qp_index_offset: i8,
    pub second_chroma_qp_index_offset: i8,
    /// `pic_fields` bit field (see [`h264_pic_fields`]).
    pub pic_fields: u32,
    pub frame_num: u16,
    pub va_reserved: [u32; VA_PADDING_MEDIUM],
}

/// The `seq_fields` bits of [`VAPictureParameterBufferH264`], low bit first.
#[allow(clippy::too_many_arguments)]
pub fn h264_seq_fields(
    chroma_format_idc: u32,
    separate_colour_plane: bool,
    gaps_in_frame_num_allowed: bool,
    frame_mbs_only: bool,
    mb_adaptive_frame_field: bool,
    direct_8x8_inference: bool,
    min_luma_bi_pred_size8x8: bool,
    log2_max_frame_num_minus4: u32,
    pic_order_cnt_type: u32,
    log2_max_poc_lsb_minus4: u32,
    delta_pic_order_always_zero: bool,
) -> u32 {
    (chroma_format_idc & 3)
        | u32::from(separate_colour_plane) << 2
        | u32::from(gaps_in_frame_num_allowed) << 3
        | u32::from(frame_mbs_only) << 4
        | u32::from(mb_adaptive_frame_field) << 5
        | u32::from(direct_8x8_inference) << 6
        | u32::from(min_luma_bi_pred_size8x8) << 7
        | (log2_max_frame_num_minus4 & 0xf) << 8
        | (pic_order_cnt_type & 3) << 12
        | (log2_max_poc_lsb_minus4 & 0xf) << 14
        | u32::from(delta_pic_order_always_zero) << 18
}

/// The `pic_fields` bits of [`VAPictureParameterBufferH264`], low bit first.
#[allow(clippy::too_many_arguments)]
pub fn h264_pic_fields(
    entropy_coding_mode: bool,
    weighted_pred: bool,
    weighted_bipred_idc: u32,
    transform_8x8_mode: bool,
    field_pic: bool,
    constrained_intra_pred: bool,
    bottom_field_pic_order_in_frame_present: bool,
    deblocking_filter_control_present: bool,
    redundant_pic_cnt_present: bool,
    reference_pic: bool,
) -> u32 {
    u32::from(entropy_coding_mode)
        | u32::from(weighted_pred) << 1
        | (weighted_bipred_idc & 3) << 2
        | u32::from(transform_8x8_mode) << 4
        | u32::from(field_pic) << 5
        | u32::from(constrained_intra_pred) << 6
        | u32::from(bottom_field_pic_order_in_frame_present) << 7
        | u32::from(deblocking_filter_control_present) << 8
        | u32::from(redundant_pic_cnt_present) << 9
        | u32::from(reference_pic) << 10
}

#[repr(C)]
#[derive(Clone, Copy, Debug)]
pub struct VAIQMatrixBufferH264 {
    pub ScalingList4x4: [[u8; 16]; 6],
    pub ScalingList8x8: [[u8; 64]; 2],
    pub va_reserved: [u32; VA_PADDING_LOW],
}

#[repr(C)]
#[derive(Clone, Copy, Debug)]
pub struct VASliceParameterBufferH264 {
    pub slice_data_size: u32,
    pub slice_data_offset: u32,
    pub slice_data_flag: u32,
    pub slice_data_bit_offset: u16,
    pub first_mb_in_slice: u16,
    pub slice_type: u8,
    pub direct_spatial_mv_pred_flag: u8,
    pub num_ref_idx_l0_active_minus1: u8,
    pub num_ref_idx_l1_active_minus1: u8,
    pub cabac_init_idc: u8,
    pub slice_qp_delta: i8,
    pub disable_deblocking_filter_idc: u8,
    pub slice_alpha_c0_offset_div2: i8,
    pub slice_beta_offset_div2: i8,
    pub RefPicList0: [VAPictureH264; 32],
    pub RefPicList1: [VAPictureH264; 32],
    pub luma_log2_weight_denom: u8,
    pub chroma_log2_weight_denom: u8,
    pub luma_weight_l0_flag: u8,
    pub luma_weight_l0: [i16; 32],
    pub luma_offset_l0: [i16; 32],
    pub chroma_weight_l0_flag: u8,
    pub chroma_weight_l0: [[i16; 2]; 32],
    pub chroma_offset_l0: [[i16; 2]; 32],
    pub luma_weight_l1_flag: u8,
    pub luma_weight_l1: [i16; 32],
    pub luma_offset_l1: [i16; 32],
    pub chroma_weight_l1_flag: u8,
    pub chroma_weight_l1: [[i16; 2]; 32],
    pub chroma_offset_l1: [[i16; 2]; 32],
    pub va_reserved: [u32; VA_PADDING_LOW],
}

// ------------------------------------------------------------------------------------- loader

type FnGetDisplayDrm = unsafe extern "C" fn(c_int) -> VADisplay;
type FnInitialize = unsafe extern "C" fn(VADisplay, *mut c_int, *mut c_int) -> VAStatus;
type FnTerminate = unsafe extern "C" fn(VADisplay) -> VAStatus;
type FnErrorStr = unsafe extern "C" fn(VAStatus) -> *const c_char;
type FnMaxNum = unsafe extern "C" fn(VADisplay) -> c_int;
type FnQueryConfigProfiles = unsafe extern "C" fn(VADisplay, *mut VAProfile, *mut c_int) -> VAStatus;
type FnQueryConfigEntrypoints = unsafe extern "C" fn(VADisplay, VAProfile, *mut VAEntrypoint, *mut c_int) -> VAStatus;
type FnGetConfigAttributes = unsafe extern "C" fn(VADisplay, VAProfile, VAEntrypoint, *mut VAConfigAttrib, c_int) -> VAStatus;
type FnCreateConfig = unsafe extern "C" fn(VADisplay, VAProfile, VAEntrypoint, *mut VAConfigAttrib, c_int, *mut VAConfigID) -> VAStatus;
type FnDestroyConfig = unsafe extern "C" fn(VADisplay, VAConfigID) -> VAStatus;
type FnCreateSurfaces = unsafe extern "C" fn(VADisplay, c_uint, c_uint, c_uint, *mut VASurfaceID, c_uint, *mut c_void, c_uint) -> VAStatus;
type FnDestroySurfaces = unsafe extern "C" fn(VADisplay, *mut VASurfaceID, c_int) -> VAStatus;
type FnCreateContext = unsafe extern "C" fn(VADisplay, VAConfigID, c_int, c_int, c_int, *mut VASurfaceID, c_int, *mut VAContextID) -> VAStatus;
type FnDestroyContext = unsafe extern "C" fn(VADisplay, VAContextID) -> VAStatus;
type FnCreateBuffer = unsafe extern "C" fn(VADisplay, VAContextID, VABufferType, c_uint, c_uint, *mut c_void, *mut VABufferID) -> VAStatus;
type FnDestroyBuffer = unsafe extern "C" fn(VADisplay, VABufferID) -> VAStatus;
type FnBeginPicture = unsafe extern "C" fn(VADisplay, VAContextID, VASurfaceID) -> VAStatus;
type FnRenderPicture = unsafe extern "C" fn(VADisplay, VAContextID, *mut VABufferID, c_int) -> VAStatus;
type FnEndPicture = unsafe extern "C" fn(VADisplay, VAContextID) -> VAStatus;
type FnSyncSurface = unsafe extern "C" fn(VADisplay, VASurfaceID) -> VAStatus;
type FnDeriveImage = unsafe extern "C" fn(VADisplay, VASurfaceID, *mut VAImage) -> VAStatus;
type FnCreateImage = unsafe extern "C" fn(VADisplay, *mut VAImageFormat, c_int, c_int, *mut VAImage) -> VAStatus;
type FnGetImage = unsafe extern "C" fn(VADisplay, VASurfaceID, c_int, c_int, c_uint, c_uint, VAImageID) -> VAStatus;
type FnDestroyImage = unsafe extern "C" fn(VADisplay, VAImageID) -> VAStatus;
type FnMapBuffer = unsafe extern "C" fn(VADisplay, VABufferID, *mut *mut c_void) -> VAStatus;
type FnUnmapBuffer = unsafe extern "C" fn(VADisplay, VABufferID) -> VAStatus;

/// The libva entry points FilmCraft uses, resolved from the loaded libraries.
pub struct Api {
    // keep the libraries loaded for as long as the function pointers are used
    _va: libloading::Library,
    _drm: libloading::Library,
    pub get_display_drm: FnGetDisplayDrm,
    pub initialize: FnInitialize,
    pub terminate: FnTerminate,
    pub error_str: FnErrorStr,
    pub max_num_profiles: FnMaxNum,
    pub max_num_entrypoints: FnMaxNum,
    pub query_config_profiles: FnQueryConfigProfiles,
    pub query_config_entrypoints: FnQueryConfigEntrypoints,
    pub get_config_attributes: FnGetConfigAttributes,
    pub create_config: FnCreateConfig,
    pub destroy_config: FnDestroyConfig,
    pub create_surfaces: FnCreateSurfaces,
    pub destroy_surfaces: FnDestroySurfaces,
    pub create_context: FnCreateContext,
    pub destroy_context: FnDestroyContext,
    pub create_buffer: FnCreateBuffer,
    pub destroy_buffer: FnDestroyBuffer,
    pub begin_picture: FnBeginPicture,
    pub render_picture: FnRenderPicture,
    pub end_picture: FnEndPicture,
    pub sync_surface: FnSyncSurface,
    pub derive_image: FnDeriveImage,
    pub create_image: FnCreateImage,
    pub get_image: FnGetImage,
    pub destroy_image: FnDestroyImage,
    pub map_buffer: FnMapBuffer,
    pub unmap_buffer: FnUnmapBuffer,
}

impl Api {
    /// Open `libva.so.2` and `libva-drm.so.2` and resolve every entry point.
    pub fn load() -> Result<Api, String> {
        // SAFETY: loading the system's libva runs its (C) initialisers, which have no
        // preconditions; the libraries stay loaded in `Api` for as long as the pointers live.
        let va = unsafe { libloading::Library::new("libva.so.2") }.map_err(|e| format!("libva.so.2: {e}"))?;
        // SAFETY: as above, for the DRM back end.
        let drm = unsafe { libloading::Library::new("libva-drm.so.2") }.map_err(|e| format!("libva-drm.so.2: {e}"))?;
        macro_rules! sym {
            ($lib:expr, $name:literal) => {{
                // SAFETY: the symbol is declared in va.h / va_drm.h with exactly the signature of
                // the type it is read as (checked by the declarations above against the header).
                let s = unsafe { $lib.get(concat!($name, "\0").as_bytes()) }.map_err(|e| format!("{}: {e}", $name))?;
                *s
            }};
        }
        Ok(Api {
            get_display_drm: sym!(drm, "vaGetDisplayDRM"),
            initialize: sym!(va, "vaInitialize"),
            terminate: sym!(va, "vaTerminate"),
            error_str: sym!(va, "vaErrorStr"),
            max_num_profiles: sym!(va, "vaMaxNumProfiles"),
            max_num_entrypoints: sym!(va, "vaMaxNumEntrypoints"),
            query_config_profiles: sym!(va, "vaQueryConfigProfiles"),
            query_config_entrypoints: sym!(va, "vaQueryConfigEntrypoints"),
            get_config_attributes: sym!(va, "vaGetConfigAttributes"),
            create_config: sym!(va, "vaCreateConfig"),
            destroy_config: sym!(va, "vaDestroyConfig"),
            create_surfaces: sym!(va, "vaCreateSurfaces"),
            destroy_surfaces: sym!(va, "vaDestroySurfaces"),
            create_context: sym!(va, "vaCreateContext"),
            destroy_context: sym!(va, "vaDestroyContext"),
            create_buffer: sym!(va, "vaCreateBuffer"),
            destroy_buffer: sym!(va, "vaDestroyBuffer"),
            begin_picture: sym!(va, "vaBeginPicture"),
            render_picture: sym!(va, "vaRenderPicture"),
            end_picture: sym!(va, "vaEndPicture"),
            sync_surface: sym!(va, "vaSyncSurface"),
            derive_image: sym!(va, "vaDeriveImage"),
            create_image: sym!(va, "vaCreateImage"),
            get_image: sym!(va, "vaGetImage"),
            destroy_image: sym!(va, "vaDestroyImage"),
            map_buffer: sym!(va, "vaMapBuffer"),
            unmap_buffer: sym!(va, "vaUnmapBuffer"),
            _va: va,
            _drm: drm,
        })
    }
}

// ------------------------------------------------------------------------------------- HEVC

pub const VA_PICTURE_HEVC_INVALID: u32 = 0x01;
pub const VA_PICTURE_HEVC_LONG_TERM_REFERENCE: u32 = 0x08;
pub const VA_PICTURE_HEVC_RPS_ST_CURR_BEFORE: u32 = 0x10;
pub const VA_PICTURE_HEVC_RPS_ST_CURR_AFTER: u32 = 0x20;
pub const VA_PICTURE_HEVC_RPS_LT_CURR: u32 = 0x40;

#[repr(C)]
#[derive(Clone, Copy, Debug, Default)]
pub struct VAPictureHEVC {
    pub picture_id: VASurfaceID,
    pub pic_order_cnt: i32,
    pub flags: u32,
    pub va_reserved: [u32; VA_PADDING_LOW],
}

impl VAPictureHEVC {
    pub const INVALID: VAPictureHEVC = VAPictureHEVC { picture_id: VA_INVALID_SURFACE, pic_order_cnt: 0, flags: VA_PICTURE_HEVC_INVALID, va_reserved: [0; VA_PADDING_LOW] };
}

#[repr(C)]
#[derive(Clone, Copy, Debug)]
pub struct VAPictureParameterBufferHEVC {
    pub CurrPic: VAPictureHEVC,
    pub ReferenceFrames: [VAPictureHEVC; 15],
    pub pic_width_in_luma_samples: u16,
    pub pic_height_in_luma_samples: u16,
    /// `pic_fields` bits, low bit first: chroma_format_idc(2), separate_colour_plane, pcm_enabled,
    /// scaling_list_enabled, transform_skip_enabled, amp_enabled, strong_intra_smoothing,
    /// sign_data_hiding, constrained_intra_pred, cu_qp_delta_enabled, weighted_pred,
    /// weighted_bipred, transquant_bypass, tiles_enabled, entropy_coding_sync,
    /// pps_loop_filter_across_slices, loop_filter_across_tiles, pcm_loop_filter_disabled,
    /// NoPicReorderingFlag, NoBiPredFlag.
    pub pic_fields: u32,
    pub sps_max_dec_pic_buffering_minus1: u8,
    pub bit_depth_luma_minus8: u8,
    pub bit_depth_chroma_minus8: u8,
    pub pcm_sample_bit_depth_luma_minus1: u8,
    pub pcm_sample_bit_depth_chroma_minus1: u8,
    pub log2_min_luma_coding_block_size_minus3: u8,
    pub log2_diff_max_min_luma_coding_block_size: u8,
    pub log2_min_transform_block_size_minus2: u8,
    pub log2_diff_max_min_transform_block_size: u8,
    pub log2_min_pcm_luma_coding_block_size_minus3: u8,
    pub log2_diff_max_min_pcm_luma_coding_block_size: u8,
    pub max_transform_hierarchy_depth_intra: u8,
    pub max_transform_hierarchy_depth_inter: u8,
    pub init_qp_minus26: i8,
    pub diff_cu_qp_delta_depth: u8,
    pub pps_cb_qp_offset: i8,
    pub pps_cr_qp_offset: i8,
    pub log2_parallel_merge_level_minus2: u8,
    pub num_tile_columns_minus1: u8,
    pub num_tile_rows_minus1: u8,
    pub column_width_minus1: [u16; 19],
    pub row_height_minus1: [u16; 21],
    /// `slice_parsing_fields` bits, low bit first: lists_modification_present,
    /// long_term_ref_pics_present, sps_temporal_mvp_enabled, cabac_init_present,
    /// output_flag_present, dependent_slice_segments_enabled,
    /// pps_slice_chroma_qp_offsets_present, sample_adaptive_offset_enabled,
    /// deblocking_filter_override_enabled, pps_disable_deblocking_filter,
    /// slice_segment_header_extension_present, RapPicFlag, IdrPicFlag, IntraPicFlag.
    pub slice_parsing_fields: u32,
    pub log2_max_pic_order_cnt_lsb_minus4: u8,
    pub num_short_term_ref_pic_sets: u8,
    pub num_long_term_ref_pic_sps: u8,
    pub num_ref_idx_l0_default_active_minus1: u8,
    pub num_ref_idx_l1_default_active_minus1: u8,
    pub pps_beta_offset_div2: i8,
    pub pps_tc_offset_div2: i8,
    pub num_extra_slice_header_bits: u8,
    pub st_rps_bits: u32,
    pub va_reserved: [u32; VA_PADDING_MEDIUM],
}

/// Pack boolean / small fields low bit first: `(value, width)` pairs.
pub fn pack_bits(fields: &[(u32, u32)]) -> u32 {
    let mut v = 0u32;
    let mut at = 0u32;
    for &(x, w) in fields {
        if at + w > 32 {
            break;
        }
        v |= (x & ((1u64 << w) - 1) as u32) << at;
        at += w;
    }
    v
}

#[repr(C)]
#[derive(Clone, Copy, Debug)]
pub struct VASliceParameterBufferHEVC {
    pub slice_data_size: u32,
    pub slice_data_offset: u32,
    pub slice_data_flag: u32,
    pub slice_data_byte_offset: u32,
    pub slice_segment_address: u32,
    pub RefPicList: [[u8; 15]; 2],
    /// `LongSliceFlags` bits, low bit first: LastSliceOfPic, dependent_slice_segment,
    /// slice_type(2), color_plane_id(2), sao_luma, sao_chroma, mvd_l1_zero, cabac_init,
    /// temporal_mvp, deblocking_disabled, collocated_from_l0, loop_filter_across_slices.
    pub LongSliceFlags: u32,
    pub collocated_ref_idx: u8,
    pub num_ref_idx_l0_active_minus1: u8,
    pub num_ref_idx_l1_active_minus1: u8,
    pub slice_qp_delta: i8,
    pub slice_cb_qp_offset: i8,
    pub slice_cr_qp_offset: i8,
    pub slice_beta_offset_div2: i8,
    pub slice_tc_offset_div2: i8,
    pub luma_log2_weight_denom: u8,
    pub delta_chroma_log2_weight_denom: i8,
    pub delta_luma_weight_l0: [i8; 15],
    pub luma_offset_l0: [i8; 15],
    pub delta_chroma_weight_l0: [[i8; 2]; 15],
    pub ChromaOffsetL0: [[i8; 2]; 15],
    pub delta_luma_weight_l1: [i8; 15],
    pub luma_offset_l1: [i8; 15],
    pub delta_chroma_weight_l1: [[i8; 2]; 15],
    pub ChromaOffsetL1: [[i8; 2]; 15],
    pub five_minus_max_num_merge_cand: u8,
    pub num_entry_point_offsets: u16,
    pub entry_offset_to_subset_array: u16,
    pub slice_data_num_emu_prevn_bytes: u16,
    pub va_reserved: [u32; VA_PADDING_LOW - 2],
}

#[repr(C)]
#[derive(Clone, Copy, Debug)]
pub struct VAIQMatrixBufferHEVC {
    pub ScalingList4x4: [[u8; 16]; 6],
    pub ScalingList8x8: [[u8; 64]; 6],
    pub ScalingList16x16: [[u8; 64]; 6],
    pub ScalingList32x32: [[u8; 64]; 2],
    pub ScalingListDC16x16: [u8; 6],
    pub ScalingListDC32x32: [u8; 2],
    pub va_reserved: [u32; VA_PADDING_LOW],
}
