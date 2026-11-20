/**
 * 사이드바 도구 모음: 폭이 좁으면 오른쪽 도구부터 "더 보기(⋯)" 메뉴로 옮긴다.
 *
 * groups[i]는 i번째 도구의 묶음 번호(왼쪽부터 순서대로). 묶음 0과 1 사이에는 구분선이 들어간다.
 * 돌려주는 값은 도구 모음에 바로 보일 도구 수. 모두 들어가면 groups.length.
 */
export const TOOL_W = 30 // 버튼 28px + 간격 2px
export const SEP_W = 11 // 구분선 1px + 좌우 여백 8px + 간격 2px
export const MORE_W = 30

export function toolbarFit(groups: number[], available: number): number {
  const widthOf = (k: number) => {
    let w = k * TOOL_W
    // 첫 묶음 뒤 구분선: 다음 묶음 도구가 하나라도 보일 때만
    if (k > 0 && groups.slice(0, k).some((g) => g > groups[0])) w += SEP_W
    return w
  }
  if (widthOf(groups.length) <= available) return groups.length
  let k = groups.length - 1
  while (k > 0 && widthOf(k) + MORE_W > available) k--
  return k
}
