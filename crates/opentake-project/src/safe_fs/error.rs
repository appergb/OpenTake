use super::capability::{EntryKind, StableIdentity};
use std::io;

pub(super) type Result<T> = std::result::Result<T, SafeFsError>;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum ComponentViolation {
    Empty,
    CurrentDirectory,
    ParentDirectory,
    AbsoluteOrPrefix,
    MultipleComponents,
    EmbeddedNul,
    TooLong,
    WindowsSeparator,
    WindowsAlternateDataStream,
    WindowsTrailingDotOrSpace,
    WindowsDeviceName,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum RelativePathViolation {
    Empty,
    AbsoluteOrPrefix,
    CurrentDirectory,
    ParentDirectory,
    InvalidComponent(ComponentViolation),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum SecureFilesystemReason {
    UnsupportedTarget,
    RemoteFilesystem,
    UnknownFilesystem,
    FilesystemProbeUnavailable,
    UnstableIdentity,
    UnstableMapping,
    CaseSemanticsUnavailable,
    VolumeChanged,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum AtomicPublishReason {
    PrimitiveUnavailable,
    FilesystemRejected,
    RemoteFilesystem,
    CrossDeviceInvariant,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum StageIdentityLostReason {
    SourceChangedBeforeQuarantine,
    QuarantinedObjectChanged,
    OriginalNameOccupied,
    QuarantineRestoreFailed,
    QuarantineNameChanged,
    ParentAuthorityChanged,
    AmbiguousNameMutation,
    CreatedObjectIdentityUnavailable,
    CreatedNameChanged,
    CreatedRollbackQuarantineFailed,
    CreatedRollbackQuarantineChanged,
    CreatedRollbackDeleteFailed,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum NativeBufferReason {
    LengthOverflow,
    DirectoryBufferTooSmall,
    DirectoryBufferMalformed,
    ReparseBufferMalformed,
    IoStatusInformationOutOfBounds,
    PendingOnSynchronousHandle,
    WriteZero,
    SecurityDescriptorMalformed,
    RenameLayoutMalformed,
    UnknownCaseFlags,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum RawOsError {
    NtStatus { status: i32, dos_error: u32 },
    Win32(u32),
    Errno(i32),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum SafeFsOperation {
    CaptureNamespaceRoot,
    OpenAncestor,
    ProbeFilesystem,
    QueryCaseMode,
    RevalidateNamespace,
    QueryChild,
    OpenDirectory,
    OpenFile,
    OpenCleanupEntry,
    QueryMetadata,
    EnumerateDirectory,
    ReadLink,
    CreateDirectory,
    CreateStageDirectory,
    CreateFile,
    RollbackCreatedEntry,
    ReadFile,
    WriteFile,
    SeekFile,
    FlushFile,
    SyncFile,
    CopyRead,
    CopyWrite,
    QuarantineNoReplace,
    VerifyQuarantine,
    RestoreQuarantine,
    DeleteQuarantinedEntry,
    DeleteQuarantinedEmptyDirectory,
    RenameNoReplaceSameParent,
    PublishNoReplace,
    ProbeVolume,
    QueryReparsePoint,
    VerifySecurityDescriptor,
    ParseDirectoryBuffer,
    ParseReparseBuffer,
}

#[derive(Debug, thiserror::Error)]
pub(crate) enum SafeFsError {
    #[error("invalid path component: {0:?}")]
    InvalidComponent(ComponentViolation),
    #[error("invalid relative path: {0:?}")]
    InvalidRelativePath(RelativePathViolation),
    #[error("entry not found during {operation:?}")]
    NotFound { operation: SafeFsOperation },
    #[error("entry already exists during {operation:?}")]
    AlreadyExists { operation: SafeFsOperation },
    #[error("symlink or reparse point during {operation:?}")]
    SymlinkOrReparsePoint { operation: SafeFsOperation },
    #[error("unsupported entry type {kind:?} during {operation:?}")]
    UnsupportedEntryType {
        operation: SafeFsOperation,
        kind: EntryKind,
    },
    #[error("identity changed during {operation:?}: expected {expected:?}, actual {actual:?}")]
    IdentityChanged {
        operation: SafeFsOperation,
        expected: StableIdentity,
        actual: StableIdentity,
    },
    #[error("namespace changed during {operation:?}")]
    NamespaceChanged { operation: SafeFsOperation },
    #[error("stage identity lost during {operation:?}: {reason:?}")]
    StageIdentityLost {
        operation: SafeFsOperation,
        reason: StageIdentityLostReason,
    },
    #[error("retained authority does not permit {operation:?}")]
    AccessMismatch { operation: SafeFsOperation },
    #[error("copy exceeded byte limit {limit}")]
    CopyLimitExceeded { limit: u64 },
    #[error("source ended before its retained size")]
    UnexpectedCopyEof,
    #[error("secure filesystem unavailable during {operation:?}: {reason:?}")]
    UnsupportedSecureFilesystem {
        operation: SafeFsOperation,
        reason: SecureFilesystemReason,
    },
    #[error("atomic publish unavailable during {operation:?}: {reason:?}")]
    UnsupportedAtomicPublish {
        operation: SafeFsOperation,
        reason: AtomicPublishReason,
    },
    #[error("filesystem I/O failed during {operation:?}: {source}")]
    Io {
        operation: SafeFsOperation,
        #[source]
        source: io::Error,
    },
    #[error("native call failed during {operation:?}: {raw:?}")]
    Os {
        operation: SafeFsOperation,
        raw: RawOsError,
    },
    #[error("invalid native buffer during {operation:?}: {reason:?}")]
    InvalidNativeBuffer {
        operation: SafeFsOperation,
        reason: NativeBufferReason,
    },
}

impl SafeFsError {
    pub(super) fn io(operation: SafeFsOperation, source: impl Into<io::Error>) -> Self {
        Self::Io {
            operation,
            source: source.into(),
        }
    }
}
