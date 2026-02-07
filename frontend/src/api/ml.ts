/**
 * ML 모델 훈련 관리 API
 *
 * Yahoo Finance 데이터로 ONNX 모델을 훈련하고 관리하는 API입니다.
 */

import axios from 'axios'

const api = axios.create({
  baseURL: '/api/v1',
  headers: {
    'Content-Type': 'application/json',
  },
})

// Auth token 인터셉터
api.interceptors.request.use((config) => {
  const token = localStorage.getItem('auth_token')
  if (token) {
    config.headers.Authorization = `Bearer ${token}`
  }
  return config
})

// ==================== 타입 정의 ====================

/** 모델 유형 */
export type ModelType = 'xgboost' | 'lightgbm' | 'random_forest' | 'gradient_boosting'

/** 훈련 상태 */
export type TrainingStatus = 'pending' | 'running' | 'completed' | 'failed'

/** 훈련 작업 */
export interface TrainingJob {
  id: string
  name: string
  modelType: ModelType
  symbols: string[]
  period: string
  horizon: number
  status: TrainingStatus
  progress: number
  startedAt: string | null
  completedAt: string | null
  error: string | null
  metrics: TrainingMetrics | null
}

/** 훈련 메트릭 */
export interface TrainingMetrics {
  accuracy: number
  auc: number
  cvAccuracy: number
  cvStd: number
  trainSamples: number
  testSamples: number
  features: number
}

/** 훈련 요청 */
export interface TrainingRequest {
  name?: string
  modelType: ModelType
  symbols: string[]
  period: string
  horizon: number
}

/** 훈련 응답 */
export interface TrainingResponse {
  success: boolean
  jobId: string
  message: string
}

/** 훈련된 모델 */
export interface TrainedModel {
  id: string
  name: string
  modelType: ModelType
  symbols: string[]
  onnxPath: string
  scalerPath: string
  createdAt: string
  metrics: TrainingMetrics
  featureNames: string[]
  isDeployed?: boolean
}

/** 모델 목록 응답 */
export interface ModelsListResponse {
  models: TrainedModel[]
  total: number
}

/** 인기 심볼 카테고리 */
export interface SymbolCategory {
  id: string
  name: string
  symbols: string[]
}

/** 인기 심볼 응답 */
export interface PopularSymbolsResponse {
  categories: SymbolCategory[]
}

// ==================== API 함수 ====================

/**
 * 훈련 작업 시작
 */
export const startTraining = async (request: TrainingRequest): Promise<TrainingResponse> => {
  const response = await api.post('/ml/train', request)
  return response.data
}

/**
 * 훈련 작업 상태 조회
 */
export const getTrainingStatus = async (jobId: string): Promise<TrainingJob> => {
  const response = await api.get(`/ml/jobs/${jobId}`)
  return response.data
}

/**
 * 훈련 작업 목록 조회
 */
export const getTrainingJobs = async (): Promise<TrainingJob[]> => {
  const response = await api.get('/ml/jobs')
  return response.data.jobs || []
}

/**
 * 훈련 작업 취소
 */
export const cancelTraining = async (jobId: string): Promise<{ success: boolean; message: string }> => {
  const response = await api.post(`/ml/jobs/${jobId}/cancel`)
  return response.data
}

/**
 * 훈련된 모델 목록 조회
 */
export const getTrainedModels = async (): Promise<ModelsListResponse> => {
  const response = await api.get('/ml/models')
  return response.data
}

/**
 * 모델 상세 조회
 */
export const getModelDetail = async (modelId: string): Promise<TrainedModel> => {
  const response = await api.get(`/ml/models/${modelId}`)
  return response.data
}

/**
 * 모델 삭제
 */
export const deleteModel = async (modelId: string): Promise<{ success: boolean; message: string }> => {
  const response = await api.delete(`/ml/models/${modelId}`)
  return response.data
}

/**
 * 모델 활성화 (추론에 사용)
 */
export const activateModel = async (modelId: string): Promise<{ success: boolean; message: string }> => {
  const response = await api.post(`/ml/models/${modelId}/activate`)
  return response.data
}

/**
 * 인기 심볼 목록 조회
 */
export const getPopularSymbols = async (): Promise<PopularSymbolsResponse> => {
  const response = await api.get('/ml/symbols/popular')
  return response.data
}

/**
 * 피처 목록 조회
 */
export const getFeatureList = async (): Promise<string[]> => {
  const response = await api.get('/ml/features')
  return response.data.features || []
}

// ==================== 유틸리티 ====================

/** 모델 유형 표시 이름 */
export const MODEL_TYPE_NAMES: Record<ModelType, string> = {
  xgboost: 'XGBoost',
  lightgbm: 'LightGBM',
  random_forest: 'Random Forest',
  gradient_boosting: 'Gradient Boosting',
}

/** 훈련 상태 표시 이름 */
export const TRAINING_STATUS_NAMES: Record<TrainingStatus, string> = {
  pending: '대기 중',
  running: '훈련 중',
  completed: '완료',
  failed: '실패',
}

/** 기간 옵션 */
export const PERIOD_OPTIONS = [
  { value: '1y', label: '1년' },
  { value: '2y', label: '2년' },
  { value: '5y', label: '5년' },
  { value: '10y', label: '10년' },
  { value: 'max', label: '전체' },
]

/** 예측 기간 옵션 */
export const HORIZON_OPTIONS = [
  { value: 1, label: '1일' },
  { value: 3, label: '3일' },
  { value: 5, label: '5일' },
  { value: 10, label: '10일' },
  { value: 20, label: '20일' },
]

export default api
