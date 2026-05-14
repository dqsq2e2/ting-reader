/// Method enum for scraper plugin calls
#[derive(Debug, Clone, Copy)]
pub enum ScraperMethod {
    Search,
    GetDetail,
    GetChapterList,
    GetChapterDetail,
    DownloadCover,
    GetAudioUrl,
}

/// Method enum for format plugin calls
#[derive(Debug, Clone, Copy)]
pub enum FormatMethod {
    Detect,
    ExtractMetadata,
    Decode,
    Encode,
    Decrypt,
    DecryptChunk,
    GetMetadataReadSize,
    GetDecryptionPlan,
    GetStreamUrl,
    WriteMetadata,
}

/// Method enum for utility plugin calls
#[derive(Debug, Clone, Copy)]
pub enum UtilityMethod {
    GetFfmpegPath,
    GetFfprobePath,
    CheckVersion,
}
