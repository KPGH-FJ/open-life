# Shared ViewModel Envelope and Types

## ViewModelEnvelope

```ts
type ViewModelEnvelope<T> = {
  data: T | null
  status: 'loading' | 'ready' | 'empty' | 'error' | 'stale'
  lastUpdatedAt: string | null
  source: 'backend-readmodel'
  evidenceRefs?: EvidenceRef[]
  warnings?: ViewModelWarning[]
  actions: {
    primary: ProductAction[]
    review?: ReviewAction[]
    debugOnly?: DebugAction[]
  }
}
```

## EvidenceRef

## ViewModelWarning

## ProductAction

## ReviewAction

## DebugAction

## ReviewItem

## ReviewItemType

## ReviewItemStatus

## ReviewItemMaterializationStatus

```ts
type ReviewItemMaterializationStatus =
  | 'not_applicable'
  | 'not_started'
  | 'applying'
  | 'applied'
  | 'failed'
  | 'rolled_back'
  | 'unknown'
```

## RiskLevel

## ImpactScope

## Empty / Error / Stale Semantics

## Ownership Rules
