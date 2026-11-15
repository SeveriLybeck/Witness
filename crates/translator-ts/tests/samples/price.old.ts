export function discount(total: number, member: boolean): number {
  if (total < 0) {
    throw new RangeError("negative total");
  }
  const base = member ? 10 : 0;
  if (total > 100) {
    return base + 5;
  }
  return base;
}
