import type { LifeStateProjection, LifeSurfaceProjection } from "../tauri";

export type LifeStateProjectionSurfaceId =
  | "today"
  | "mailbox"
  | "chat"
  | "companion"
  | "life_model"
  | "settings";

export function findLifeStateSurface(
  projection: LifeStateProjection | null,
  surface: LifeStateProjectionSurfaceId
): LifeSurfaceProjection | null {
  if (!projection) return null;
  return projection.surfaces.find(item => item.surface === surface) ?? null;
}

export function reviewRequiredCountFromProjection(
  projection: LifeStateProjection | null,
  surface: LifeStateProjectionSurfaceId
): number | null {
  if (!projection) return null;
  return (
    findLifeStateSurface(projection, surface)?.totalReviewRequiredCount ??
    projection.pending.totalReviewRequiredCount
  );
}
