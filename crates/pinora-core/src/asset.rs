//! 截图资产的版本引用。
//!
//! 异步任务只可在其结果仍对应当前资产版本时提交，避免陈旧结果回写。

use std::num::NonZeroU64;

use crate::ids::ImageId;

/// 不可为零且单调递增的资产版本。
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct AssetGeneration(NonZeroU64);

impl AssetGeneration {
    /// 新资产的初始版本。
    pub const INITIAL: Self = Self(NonZeroU64::MIN);

    /// 从持久化或协议值恢复版本；零不是有效版本。
    pub const fn from_raw(value: u64) -> Option<Self> {
        match NonZeroU64::new(value) {
            Some(value) => Some(Self(value)),
            None => None,
        }
    }

    /// 返回下一个版本；到达最大值时不回绕。
    pub fn advance(self) -> Option<Self> {
        self.0.checked_add(1).map(Self)
    }

    pub const fn raw(self) -> u64 {
        self.0.get()
    }
}

impl Default for AssetGeneration {
    fn default() -> Self {
        Self::INITIAL
    }
}

/// 绑定图像 ID 与其当前 generation 的不可变引用。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct AssetRef {
    pub image_id: ImageId,
    pub generation: AssetGeneration,
}

impl AssetRef {
    pub const fn new(image_id: ImageId, generation: AssetGeneration) -> Self {
        Self {
            image_id,
            generation,
        }
    }

    pub const fn initial(image_id: ImageId) -> Self {
        Self::new(image_id, AssetGeneration::INITIAL)
    }

    /// 创建同一资产的下一版本；到达版本上限时返回 `None`，绝不回绕。
    pub fn advance(self) -> Option<Self> {
        self.generation.advance().map(|generation| Self {
            image_id: self.image_id,
            generation,
        })
    }

    /// 当前资产是否接受某任务结果。
    ///
    /// 只有图像 ID 与 generation 都相等时，结果才可提交。
    pub fn accepts_result(self, result: Self) -> bool {
        self == result
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn generation_starts_non_zero_and_advances_monotonically() {
        let initial = AssetGeneration::INITIAL;
        let next = initial.advance().expect("initial generation advances");

        assert_eq!(initial.raw(), 1);
        assert_eq!(next.raw(), 2);
        assert!(next > initial);
        assert_eq!(AssetGeneration::from_raw(0), None);
    }

    #[test]
    fn generation_never_wraps() {
        let maximum = AssetGeneration::from_raw(u64::MAX).expect("non-zero maximum");

        assert_eq!(maximum.advance(), None);
    }

    #[test]
    fn current_asset_accepts_only_matching_result_reference() {
        let image = ImageId::from_raw(7);
        let current = AssetRef::initial(image)
            .advance()
            .expect("initial asset reference advances");

        assert!(current.accepts_result(current));
        assert!(!current.accepts_result(AssetRef::initial(image)));
        assert!(!current.accepts_result(AssetRef::new(ImageId::from_raw(8), current.generation,)));
    }
}
