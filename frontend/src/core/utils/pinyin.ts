export const getPinyinInitial = (str: string): string => {
  if (!str) return '#';
  const char = str[0];
  
  // Latin characters
  if (/^[a-zA-Z]/.test(char)) {
    return char.toUpperCase();
  }
  
  // Check for Chinese characters
  if (!/[\u4e00-\u9fa5]/.test(char)) {
    return '#';
  }

  // Comparison based on zh-CN collation
  // Boundaries for Pinyin initials
  const boundaries = [
    { char: '阿', letter: 'A' },
    { char: '芭', letter: 'B' },
    { char: '擦', letter: 'C' },
    { char: '搭', letter: 'D' },
    { char: '婀', letter: 'E' },
    { char: '发', letter: 'F' },
    { char: '噶', letter: 'G' },
    { char: '哈', letter: 'H' },
    { char: '肌', letter: 'J' },
    { char: '喀', letter: 'K' },
    { char: '垃', letter: 'L' },
    { char: '妈', letter: 'M' },
    { char: '拿', letter: 'N' },
    { char: '哦', letter: 'O' },
    { char: '啪', letter: 'P' },
    { char: '期', letter: 'Q' },
    { char: '然', letter: 'R' },
    { char: '撒', letter: 'S' },
    { char: '塌', letter: 'T' },
    { char: '挖', letter: 'W' },
    { char: '昔', letter: 'X' },
    { char: '压', letter: 'Y' },
    { char: '匝', letter: 'Z' },
  ];

  for (let i = boundaries.length - 1; i >= 0; i--) {
    if (char.localeCompare(boundaries[i].char, 'zh-CN') >= 0) {
      return boundaries[i].letter;
    }
  }

  return '#';
};
