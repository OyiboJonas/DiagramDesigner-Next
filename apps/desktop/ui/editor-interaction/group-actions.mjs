export function isGroupActionEnabled({ canGroup = false, busy = false } = {}) {
  return busy !== true && canGroup === true;
}

export function isUngroupActionEnabled({ canUngroup = false, busy = false } = {}) {
  return busy !== true && canUngroup === true;
}
