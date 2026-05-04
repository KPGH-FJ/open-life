export interface Metadata {
  version: string;
  created_at: string;
  updated_at: string;
  author: string;
}

export interface ValueItem {
  name: string;
  weight: number;
  description: string;
}

export interface PersonalityTrait {
  trait_name: string;
  score: number;
}

export interface RoleDefinition {
  primary_role: string;
  secondary_roles: string[];
  responsibilities: string[];
  boundaries: string[];
}

export type FormalityLevel = "casual" | "neutral" | "formal";
export type EmojiUsage = "never" | "sparingly" | "often";

export interface VoiceStyle {
  formality: FormalityLevel;
  tone_descriptors: string[];
  vocabulary_preference: string;
  emoji_usage: EmojiUsage;
}

export interface Identity {
  name: string;
  birth_date?: string;
  values: ValueItem[];
  personality_traits: PersonalityTrait[];
  life_philosophy: string;
  mission_statement: string;
  role_definition: RoleDefinition;
  voice_style: VoiceStyle;
}

export interface Milestone {
  name: string;
  target_date?: string;
  status: string;
  description: string;
}

export interface GoalItem {
  name: string;
  priority: number;
  status: string;
  milestones: Milestone[];
  description: string;
  progress: number;
  related_memories: string[];
}

export interface DailyGoal {
  name: string;
  done: boolean;
  time_block?: {
    start: string;
    end: string;
  };
}

export interface Goals {
  short_term: GoalItem[];
  medium_term: GoalItem[];
  long_term: GoalItem[];
  life_goals: GoalItem[];
  daily: DailyGoal[];
  progress: number;
  related_memories: string[];
}

export interface Skill {
  name: string;
  proficiency: number;
  description: string;
}

export interface Resource {
  name: string;
  resource_type: string;
  description: string;
  availability: string;
}

export interface ToolCapability {
  name: string;
  proficiency: number;
  description: string;
}

export interface KnowledgeDomain {
  domain: string;
  level: number;
  description: string;
}

export interface Capabilities {
  skills: Skill[];
  resources: Resource[];
  networks: string[];
  tools: ToolCapability[];
  knowledge_domains: KnowledgeDomain[];
}

export interface HealthStatus {
  physical: string;
  mental: string;
  energy_level: number;
}

export interface EmotionalState {
  current_mood: string;
  stress_level: number;
  fulfillment_score: number;
}

export interface Reflection {
  date: string;
  content: string;
  insights: string[];
}

export interface HabitStreak {
  name: string;
  streak_days: number;
}

export interface CustomStateDimension {
  name: string;
  unit: string;
  current_value: number;
  min_threshold?: number;
  max_threshold?: number;
  alert_days: number;
}

export type AlertLevel = "info" | "warning" | "critical";

export interface StateAlert {
  dimension_name: string;
  level: AlertLevel;
  message: string;
  triggered_at: string;
}

export interface State {
  current_focus: string;
  health_status: HealthStatus;
  emotional_state: EmotionalState;
  recent_reflections: Reflection[];
  open_questions: string[];
  focus_areas: string[];
  recent_events: string[];
  habit_streaks: HabitStreak[];
  custom_dimensions: CustomStateDimension[];
  alerts: StateAlert[];
}

export interface StateHistoryEntry {
  id: number;
  dimension_name: string;
  value: number;
  unit: string;
  recorded_at: string;
  note?: string;
}

export interface Relationship {
  name: string;
  relationship_type: string;
  importance: number;
  notes: string;
}

export interface Relationships {
  inner_circle: Relationship[];
  mentors: Relationship[];
  collaborators: Relationship[];
}

export interface WorkHours {
  preferred_start: string;
  preferred_end: string;
  timezone: string;
}

export interface Preferences {
  work_hours: WorkHours;
  peak_energy_time: string;
  communication_style: string;
  learning_style: string;
  decision_making_style: string;
}

export interface LifeModel {
  metadata: Metadata;
  identity: Identity;
  goals: Goals;
  capabilities: Capabilities;
  state: State;
  relationships: Relationships;
  preferences: Preferences;
  evolution_rules: string[];
}

export interface BuilderProgress {
  progress: number;
  current_step_label: string;
  step_index: number;
  total_steps: number;
  current_session?: number;
  waiting_pairwise?: boolean;
  waiting_phase_confirmation?: boolean;
  phase_summary?: string;
}

export interface ChatMessage {
  role: "system" | "user" | "assistant";
  content: string;
  run_id?: string;
}

export interface LifeModelVersion {
  version: string;
  timestamp: string;
  tag: string;
  note: string;
  yaml_content: string;
}
