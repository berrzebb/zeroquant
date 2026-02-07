import { createSignal, For, Show, createResource, createEffect, onCleanup } from 'solid-js'
import { Save, Key, Bell, Database, Globe, Moon, Sun, Send, MessageCircle, CheckCircle, XCircle, RefreshCw, Play, Plus, Trash2, TestTube, Building2, Bot, ChevronDown, ChevronRight, BellRing } from 'lucide-solid'
import { useToast } from '../components/Toast'
import {
  Card,
  CardHeader,
  CardContent,
  Button,
} from '../components/ui'
import {
  getNotificationSettings,
  getNotificationTemplates,
  testTelegram,
  testTelegramEnv,
  testTelegramTemplate,
  testAllTelegramTemplates,
  getSupportedExchanges,
  listCredentials,
  createCredential,
  deleteCredential,
  testNewCredential,
  testExistingCredential,
  getTelegramSettings,
  saveTelegramSettings,
  deleteTelegramSettings,
  getActiveAccount,
  setActiveAccount,
  // Discord API
  getDiscordSettings,
  saveDiscordSettings,
  deleteDiscordSettings,
  testDiscordSettings,
  testNewDiscordSettings,
  // Slack API
  getSlackSettings,
  saveSlackSettings,
  deleteSlackSettings,
  testSlackSettings,
  testNewSlackSettings,
  // Email API
  getEmailSettings,
  saveEmailSettings,
  deleteEmailSettings,
  testEmailSettings,
  testNewEmailSettings,
  // SMS API
  getSmsSettings,
  saveSmsSettings,
  deleteSmsSettings,
  testSmsSettings,
  testNewSmsSettings,
  type TelegramTestResponse,
  type CredentialTestResponse,
  type ActiveAccount,
} from '../api/client'
import type { SupportedExchange } from '../types'

// 알림 서비스 프로바이더 타입
interface NotificationProvider {
  id: string
  name: string
  icon: string
  description: string
  fields: Array<{
    name: string
    label: string
    type: 'text' | 'password'
    placeholder: string
    helpText?: string
  }>
}

// 지원되는 알림 서비스 목록
const NOTIFICATION_PROVIDERS: NotificationProvider[] = [
  {
    id: 'telegram',
    name: 'Telegram',
    icon: '📱',
    description: '텔레그램 봇을 통한 알림',
    fields: [
      {
        name: 'bot_token',
        label: 'Bot Token',
        type: 'password',
        placeholder: '123456789:ABCdefGHIjklMNOpqrsTUVwxyz',
        helpText: '@BotFather에서 발급받은 Bot Token'
      },
      {
        name: 'chat_id',
        label: 'Chat ID',
        type: 'text',
        placeholder: '-1001234567890',
        helpText: '@userinfobot 또는 @getidsbot에서 확인'
      }
    ]
  },
  {
    id: 'discord',
    name: 'Discord',
    icon: '🎮',
    description: 'Discord Webhook을 통한 알림',
    fields: [
      {
        name: 'webhook_url',
        label: 'Webhook URL',
        type: 'password',
        placeholder: 'https://discord.com/api/webhooks/...',
        helpText: '서버 설정 > 연동 > 웹훅에서 생성'
      },
      {
        name: 'display_name',
        label: '봇 이름 (선택)',
        type: 'text',
        placeholder: 'ZeroQuant Alerts',
        helpText: '알림 발송 시 표시될 이름'
      }
    ]
  },
  {
    id: 'slack',
    name: 'Slack',
    icon: '💬',
    description: 'Slack Incoming Webhook을 통한 알림',
    fields: [
      {
        name: 'webhook_url',
        label: 'Webhook URL',
        type: 'password',
        placeholder: 'https://hooks.slack.com/services/...',
        helpText: 'Slack App > Incoming Webhooks에서 생성'
      },
      {
        name: 'display_name',
        label: '표시 이름 (선택)',
        type: 'text',
        placeholder: 'ZeroQuant Alerts',
        helpText: '관리 목적의 표시 이름'
      }
    ]
  },
  {
    id: 'email',
    name: 'Email',
    icon: '📧',
    description: 'SMTP를 통한 이메일 알림',
    fields: [
      {
        name: 'smtp_host',
        label: 'SMTP 서버',
        type: 'text',
        placeholder: 'smtp.gmail.com',
        helpText: 'SMTP 서버 주소 (Gmail: smtp.gmail.com)'
      },
      {
        name: 'smtp_port',
        label: 'SMTP 포트',
        type: 'text',
        placeholder: '587',
        helpText: 'TLS: 587, SSL: 465'
      },
      {
        name: 'username',
        label: '사용자명',
        type: 'text',
        placeholder: 'your-email@gmail.com',
        helpText: 'SMTP 로그인 사용자명'
      },
      {
        name: 'password',
        label: '비밀번호',
        type: 'password',
        placeholder: 'App Password',
        helpText: 'Gmail의 경우 앱 비밀번호 사용'
      },
      {
        name: 'from_email',
        label: '발신 이메일',
        type: 'text',
        placeholder: 'alerts@example.com',
        helpText: '알림 발송에 사용될 이메일 주소'
      },
      {
        name: 'to_emails',
        label: '수신 이메일',
        type: 'text',
        placeholder: 'user@example.com',
        helpText: '알림을 받을 이메일 (콤마로 구분)'
      }
    ]
  },
  {
    id: 'sms',
    name: 'SMS (Twilio)',
    icon: '📲',
    description: 'Twilio를 통한 SMS 알림',
    fields: [
      {
        name: 'account_sid',
        label: 'Account SID',
        type: 'text',
        placeholder: 'ACxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxx',
        helpText: 'Twilio Console에서 확인'
      },
      {
        name: 'auth_token',
        label: 'Auth Token',
        type: 'password',
        placeholder: 'xxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxx',
        helpText: 'Twilio Console에서 확인'
      },
      {
        name: 'from_number',
        label: '발신 번호',
        type: 'text',
        placeholder: '+15551234567',
        helpText: 'Twilio 전화번호 (E.164 형식)'
      },
      {
        name: 'to_numbers',
        label: '수신 번호',
        type: 'text',
        placeholder: '+821012345678',
        helpText: '알림을 받을 전화번호 (콤마로 구분)'
      }
    ]
  }
]

export function Settings() {
  // Toast 알림
  const toast = useToast()

  // 저장 타이머 cleanup 추적
  let saveTimeout: ReturnType<typeof setTimeout> | null = null
  onCleanup(() => {
    if (saveTimeout) clearTimeout(saveTimeout)
  })

  // 알림 설정 리소스
  const [notificationSettings, { refetch: refetchNotificationSettings }] = createResource(async () => {
    try {
      return await getNotificationSettings()
    } catch {
      return { telegram_enabled: false, telegram_configured: false }
    }
  })

  // 템플릿 목록 리소스
  const [templates] = createResource(async () => {
    try {
      const response = await getNotificationTemplates()
      return response.templates
    } catch {
      return []
    }
  })

  // ==================== 거래소 자격증명 관리 ====================
  // 지원되는 거래소 목록
  const [exchanges] = createResource(async () => {
    try {
      const response = await getSupportedExchanges()
      return response.exchanges
    } catch {
      return []
    }
  })

  // 등록된 자격증명 목록
  const [credentials, { refetch: refetchCredentials }] = createResource(async () => {
    try {
      const response = await listCredentials()
      return response.credentials
    } catch {
      return []
    }
  })

  // 활성 계정 상태
  const [activeAccount, { refetch: refetchActiveAccount }] = createResource(async () => {
    try {
      return await getActiveAccount()
    } catch {
      return { credential_id: null, exchange_id: null, display_name: null, is_testnet: false }
    }
  })
  const [isSettingActiveAccount, setIsSettingActiveAccount] = createSignal(false)

  // 활성 계정 변경
  const handleSetActiveAccount = async (credentialId: string | null) => {
    setIsSettingActiveAccount(true)
    try {
      const result = await setActiveAccount(credentialId)
      if (result.success) {
        refetchActiveAccount()
      } else {
        toast.error('계정 변경 실패', result.message)
      }
    } catch {
      toast.error('계정 변경 실패', '서버 연결 오류')
    } finally {
      setIsSettingActiveAccount(false)
    }
  }

  // 자격증명 폼 상태
  const [showCredentialForm, setShowCredentialForm] = createSignal(false)
  const [selectedExchange, setSelectedExchange] = createSignal<SupportedExchange | null>(null)
  const [credentialFields, setCredentialFields] = createSignal<Record<string, string>>({})
  const [credentialDisplayName, setCredentialDisplayName] = createSignal('')
  const [isTestnet, setIsTestnet] = createSignal(false)  // 모의투자/테스트넷 여부
  const [isCredentialTesting, setIsCredentialTesting] = createSignal(false)
  const [isCredentialSaving, setIsCredentialSaving] = createSignal(false)
  const [credentialTestResult, setCredentialTestResult] = createSignal<CredentialTestResponse | null>(null)
  const [deletingCredentialId, setDeletingCredentialId] = createSignal<string | null>(null)

  // 거래소 선택 시 필드 초기화
  const handleExchangeSelect = (exchangeId: string) => {
    const exchange = exchanges()?.find(e => e.exchange_id === exchangeId)
    setSelectedExchange(exchange || null)
    setCredentialFields({})
    setCredentialDisplayName(exchange?.display_name || '')
    setIsTestnet(false)  // 모의투자 선택 초기화
    setCredentialTestResult(null)
  }

  // 필드 값 업데이트
  const updateField = (fieldName: string, value: string) => {
    setCredentialFields(prev => ({ ...prev, [fieldName]: value }))
  }

  // 자격증명 테스트
  const handleCredentialTest = async () => {
    const exchange = selectedExchange()
    if (!exchange) return

    setIsCredentialTesting(true)
    setCredentialTestResult(null)

    try {
      const result = await testNewCredential({
        exchange_id: exchange.exchange_id,
        fields: credentialFields()
      })
      setCredentialTestResult(result)
    } catch (err) {
      setCredentialTestResult({
        success: false,
        message: '테스트 실패: 서버 연결 오류'
      })
    } finally {
      setIsCredentialTesting(false)
    }
  }

  // 자격증명 저장
  const handleCredentialSave = async () => {
    const exchange = selectedExchange()
    if (!exchange) return

    setIsCredentialSaving(true)

    try {
      const result = await createCredential({
        exchange_id: exchange.exchange_id,
        display_name: credentialDisplayName() || exchange.display_name,
        fields: credentialFields(),
        is_testnet: isTestnet()  // 모의투자 여부 포함
      })

      if (result.success) {
        setShowCredentialForm(false)
        setSelectedExchange(null)
        setCredentialFields({})
        setCredentialDisplayName('')
        setIsTestnet(false)  // 초기화
        setCredentialTestResult(null)
        refetchCredentials()
      } else {
        setCredentialTestResult({
          success: false,
          message: result.message || '저장 실패'
        })
      }
    } catch (err) {
      setCredentialTestResult({
        success: false,
        message: '저장 실패: 서버 연결 오류'
      })
    } finally {
      setIsCredentialSaving(false)
    }
  }

  // 자격증명 삭제
  const handleCredentialDelete = async (id: string) => {
    if (!confirm('이 API 키를 삭제하시겠습니까?')) return

    setDeletingCredentialId(id)

    try {
      await deleteCredential(id)
      refetchCredentials()
      toast.success('삭제 완료', 'API 키가 삭제되었습니다.')
    } catch (err) {
      toast.error('삭제 실패', '서버 연결 오류')
    } finally {
      setDeletingCredentialId(null)
    }
  }

  // 기존 자격증명 테스트
  const handleExistingCredentialTest = async (id: string) => {
    try {
      const result = await testExistingCredential(id)
      if (result.success) {
        toast.success('연결 테스트 성공', '거래소와 정상적으로 연결되었습니다.')
      } else {
        toast.error('테스트 실패', result.message)
      }
    } catch {
      toast.error('테스트 실패', '서버 연결 오류')
    }
  }

  // ==================== API 키 관리 탭 ====================
  type ApiKeyTab = 'exchange' | 'notification'
  const [activeApiTab, setActiveApiTab] = createSignal<ApiKeyTab>('exchange')

  // ==================== 알림 서비스 관리 ====================
  // 통합 알림 서비스 타입
  interface NotificationServiceItem {
    id: string;
    provider_id: string;
    display_name: string;
    is_active: boolean;
    created_at: string;
    last_tested_at?: string;
    masked_info: string;  // 마스킹된 정보 (토큰, URL 등)
  }

  // 등록된 알림 서비스 목록 (모든 프로바이더 통합 조회)
  const [notificationServices, { refetch: refetchNotificationServices }] = createResource(async () => {
    const services: NotificationServiceItem[] = []

    // Telegram 설정 조회
    try {
      const telegram = await getTelegramSettings()
      if (telegram.configured) {
        services.push({
          id: 'telegram-default',
          provider_id: 'telegram',
          display_name: telegram.display_name || 'Telegram',
          is_active: true,
          created_at: telegram.created_at || new Date().toISOString(),
          last_tested_at: telegram.last_tested_at,
          masked_info: `Token: ${telegram.masked_token || '****'}`,
        })
      }
    } catch { /* 설정 없음 */ }

    // Discord 설정 조회
    try {
      const discord = await getDiscordSettings()
      if (discord.configured) {
        services.push({
          id: 'discord-default',
          provider_id: 'discord',
          display_name: discord.display_name || 'Discord',
          is_active: discord.is_enabled,
          created_at: discord.created_at || new Date().toISOString(),
          last_tested_at: discord.last_verified_at,
          masked_info: `Webhook: ${discord.webhook_url_masked || '****'}`,
        })
      }
    } catch { /* 설정 없음 */ }

    // Slack 설정 조회
    try {
      const slack = await getSlackSettings()
      if (slack.configured) {
        services.push({
          id: 'slack-default',
          provider_id: 'slack',
          display_name: slack.display_name || 'Slack',
          is_active: slack.is_enabled,
          created_at: slack.created_at || new Date().toISOString(),
          last_tested_at: slack.last_verified_at,
          masked_info: `Webhook: ${slack.webhook_url_masked || '****'}`,
        })
      }
    } catch { /* 설정 없음 */ }

    // Email 설정 조회
    try {
      const email = await getEmailSettings()
      if (email.configured) {
        services.push({
          id: 'email-default',
          provider_id: 'email',
          display_name: 'Email',
          is_active: email.is_enabled,
          created_at: email.created_at || new Date().toISOString(),
          last_tested_at: email.last_verified_at,
          masked_info: `SMTP: ${email.smtp_host}:${email.smtp_port}`,
        })
      }
    } catch { /* 설정 없음 */ }

    // SMS 설정 조회
    try {
      const sms = await getSmsSettings()
      if (sms.configured) {
        services.push({
          id: 'sms-default',
          provider_id: 'sms',
          display_name: `SMS (${sms.provider})`,
          is_active: sms.is_enabled,
          created_at: sms.created_at || new Date().toISOString(),
          last_tested_at: sms.last_verified_at,
          masked_info: `From: ${sms.from_number}`,
        })
      }
    } catch { /* 설정 없음 */ }

    return services
  })

  // 알림 서비스 추가 폼 상태
  const [showNotificationForm, setShowNotificationForm] = createSignal(false)
  const [selectedProvider, setSelectedProvider] = createSignal<NotificationProvider | null>(null)
  const [notificationFields, setNotificationFields] = createSignal<Record<string, string>>({})
  const [notificationDisplayName, setNotificationDisplayName] = createSignal('')
  const [isNotificationTesting, setIsNotificationTesting] = createSignal(false)
  const [isNotificationSaving, setIsNotificationSaving] = createSignal(false)
  const [notificationTestResult, setNotificationTestResult] = createSignal<TelegramTestResponse | null>(null)
  const [deletingNotificationId, setDeletingNotificationId] = createSignal<string | null>(null)

  // 알림 프로바이더 선택
  const handleProviderSelect = (providerId: string) => {
    const provider = NOTIFICATION_PROVIDERS.find(p => p.id === providerId)
    setSelectedProvider(provider || null)
    setNotificationFields({})
    setNotificationDisplayName(provider?.name || '')
    setNotificationTestResult(null)
  }

  // 알림 필드 값 업데이트
  const updateNotificationField = (fieldName: string, value: string) => {
    setNotificationFields(prev => ({ ...prev, [fieldName]: value }))
  }

  // 알림 서비스 테스트 (저장 전 새 설정 테스트)
  const handleNotificationTest = async () => {
    const provider = selectedProvider()
    if (!provider) return

    setIsNotificationTesting(true)
    setNotificationTestResult(null)

    try {
      const fields = notificationFields()
      let result: TelegramTestResponse = { success: false, message: '알 수 없는 프로바이더' }

      switch (provider.id) {
        case 'telegram':
          result = await testTelegram({
            bot_token: fields['bot_token'] || '',
            chat_id: fields['chat_id'] || ''
          })
          break

        case 'discord':
          // 실제 Discord 테스트 메시지 전송
          result = await testNewDiscordSettings({
            webhook_url: fields['webhook_url'] || '',
            display_name: notificationDisplayName() || fields['display_name'] || undefined,
          })
          break

        case 'slack':
          // 실제 Slack 테스트 메시지 전송
          result = await testNewSlackSettings({
            webhook_url: fields['webhook_url'] || '',
            display_name: notificationDisplayName() || fields['display_name'] || undefined,
          })
          break

        case 'email':
          // 실제 이메일 테스트 전송
          result = await testNewEmailSettings({
            smtp_host: fields['smtp_host'] || '',
            smtp_port: parseInt(fields['smtp_port'] || '587', 10),
            use_tls: true,
            username: fields['username'] || '',
            password: fields['password'] || '',
            from_email: fields['from_email'] || '',
            from_name: notificationDisplayName() || undefined,
            to_emails: (fields['to_emails'] || '').split(',').map(e => e.trim()).filter(Boolean),
          })
          break

        case 'sms':
          // 실제 SMS 테스트 전송
          result = await testNewSmsSettings({
            provider: 'twilio',
            account_sid: fields['account_sid'] || '',
            auth_token: fields['auth_token'] || '',
            from_number: fields['from_number'] || '',
            to_numbers: (fields['to_numbers'] || '').split(',').map(n => n.trim()).filter(Boolean),
          })
          break
      }

      setNotificationTestResult(result)
    } catch (err: unknown) {
      // 서버 에러 메시지 추출
      let errorMessage = '테스트 실패: 서버 연결 오류'
      if (err && typeof err === 'object' && 'response' in err) {
        const axiosError = err as { response?: { data?: { message?: string } } }
        if (axiosError.response?.data?.message) {
          errorMessage = axiosError.response.data.message
        }
      }
      setNotificationTestResult({
        success: false,
        message: errorMessage
      })
    } finally {
      setIsNotificationTesting(false)
    }
  }

  // 알림 서비스 저장
  const handleNotificationSave = async () => {
    const provider = selectedProvider()
    if (!provider) return

    setIsNotificationSaving(true)

    try {
      const fields = notificationFields()
      let result: { success: boolean; message: string } = { success: false, message: '알 수 없는 프로바이더' }

      switch (provider.id) {
        case 'telegram':
          result = await saveTelegramSettings({
            bot_token: fields['bot_token'] || '',
            chat_id: fields['chat_id'] || '',
            display_name: notificationDisplayName() || 'Telegram'
          })
          break

        case 'discord':
          result = await saveDiscordSettings({
            webhook_url: fields['webhook_url'] || '',
            display_name: notificationDisplayName() || fields['display_name'] || undefined,
          })
          break

        case 'slack':
          result = await saveSlackSettings({
            webhook_url: fields['webhook_url'] || '',
            display_name: notificationDisplayName() || fields['display_name'] || undefined,
          })
          break

        case 'email':
          result = await saveEmailSettings({
            smtp_host: fields['smtp_host'] || '',
            smtp_port: parseInt(fields['smtp_port'] || '587', 10),
            use_tls: true,
            username: fields['username'] || '',
            password: fields['password'] || '',
            from_email: fields['from_email'] || '',
            from_name: notificationDisplayName() || undefined,
            to_emails: (fields['to_emails'] || '').split(',').map(e => e.trim()).filter(Boolean),
          })
          break

        case 'sms':
          result = await saveSmsSettings({
            provider: 'twilio',
            account_sid: fields['account_sid'] || '',
            auth_token: fields['auth_token'] || '',
            from_number: fields['from_number'] || '',
            to_numbers: (fields['to_numbers'] || '').split(',').map(n => n.trim()).filter(Boolean),
          })
          break
      }

      if (result.success) {
        setShowNotificationForm(false)
        setSelectedProvider(null)
        setNotificationFields({})
        setNotificationDisplayName('')
        setNotificationTestResult(null)
        refetchNotificationServices()
        refetchNotificationSettings()
        toast.success('저장 완료', `${provider.name} 설정이 저장되었습니다.`)
      } else {
        setNotificationTestResult({
          success: false,
          message: result.message || '저장 실패'
        })
      }
    } catch (err) {
      setNotificationTestResult({
        success: false,
        message: '저장 실패: 서버 연결 오류'
      })
    } finally {
      setIsNotificationSaving(false)
    }
  }

  // 알림 서비스 삭제
  const handleNotificationDelete = async (id: string, providerId: string) => {
    const provider = NOTIFICATION_PROVIDERS.find(p => p.id === providerId)
    if (!confirm(`${provider?.name || '알림 서비스'}를 삭제하시겠습니까?`)) return

    setDeletingNotificationId(id)

    try {
      switch (providerId) {
        case 'telegram':
          await deleteTelegramSettings()
          break
        case 'discord':
          await deleteDiscordSettings()
          break
        case 'slack':
          await deleteSlackSettings()
          break
        case 'email':
          await deleteEmailSettings()
          break
        case 'sms':
          await deleteSmsSettings()
          break
      }
      refetchNotificationServices()
      refetchNotificationSettings()
      toast.success('삭제 완료', `${provider?.name || '알림 서비스'}가 삭제되었습니다.`)
    } catch (err) {
      toast.error('삭제 실패', '서버 연결 오류')
    } finally {
      setDeletingNotificationId(null)
    }
  }

  // 기존 알림 서비스 테스트
  const handleExistingNotificationTest = async (id: string, providerId: string) => {
    const provider = NOTIFICATION_PROVIDERS.find(p => p.id === providerId)

    try {
      let result: { success: boolean; message: string } = { success: false, message: '알 수 없는 프로바이더' }

      switch (providerId) {
        case 'telegram':
          result = await testTelegramEnv()
          break
        case 'discord':
          result = await testDiscordSettings()
          break
        case 'slack':
          result = await testSlackSettings()
          break
        case 'email':
          result = await testEmailSettings()
          break
        case 'sms':
          result = await testSmsSettings()
          break
      }

      if (result.success) {
        toast.success('연결 테스트 성공', `${provider?.name}과(와) 정상적으로 연결되었습니다.`)
      } else {
        toast.error('테스트 실패', result.message)
      }
    } catch {
      toast.error('테스트 실패', '서버 연결 오류')
    }
  }

  // ==================== 기타 설정 ====================
  const [isDarkMode, setIsDarkMode] = createSignal(true)
  const [notifications, setNotifications] = createSignal({
    tradeExecution: true,
    priceAlerts: true,
    dailyReport: false,
    errorAlerts: true,
  })
  const [telegramSettings, setTelegramSettings] = createSignal({
    botToken: '',
    chatId: '',
    isConnected: false,
  })
  const [isTelegramTesting, setIsTelegramTesting] = createSignal(false)
  const [telegramTestResult, setTelegramTestResult] = createSignal<TelegramTestResponse | null>(null)
  const [selectedTemplate, setSelectedTemplate] = createSignal<string>('')
  const [isTemplateTesting, setIsTemplateTesting] = createSignal(false)
  const [isSaving, setIsSaving] = createSignal(false)

  // 서버에 저장된 텔레그램 설정이 있으면 연결 상태 업데이트
  createEffect(() => {
    const settings = notificationSettings()
    if (settings?.telegram_configured) {
      setTelegramSettings(prev => ({ ...prev, isConnected: true }))
    }
  })

  // 텔레그램 연결 테스트 (직접 입력한 토큰으로)
  const handleTelegramTest = async () => {
    const { botToken, chatId } = telegramSettings()

    if (!botToken || !chatId) {
      setTelegramTestResult({
        success: false,
        message: 'Bot Token과 Chat ID를 모두 입력해주세요.'
      })
      return
    }

    setIsTelegramTesting(true)
    setTelegramTestResult(null)

    try {
      const result = await testTelegram({ bot_token: botToken, chat_id: chatId })
      setTelegramTestResult(result)
      setTelegramSettings(prev => ({ ...prev, isConnected: result.success }))
    } catch (err) {
      setTelegramTestResult({
        success: false,
        message: '서버 연결에 실패했습니다. 나중에 다시 시도해주세요.'
      })
    } finally {
      setIsTelegramTesting(false)
    }
  }

  // 환경변수로 설정된 텔레그램 테스트
  const handleTelegramEnvTest = async () => {
    setIsTelegramTesting(true)
    setTelegramTestResult(null)

    try {
      const result = await testTelegramEnv()
      setTelegramTestResult(result)
      if (result.success) {
        setTelegramSettings(prev => ({ ...prev, isConnected: true }))
        refetchNotificationSettings()
      }
    } catch (err) {
      setTelegramTestResult({
        success: false,
        message: '서버 연결에 실패했습니다.'
      })
    } finally {
      setIsTelegramTesting(false)
    }
  }

  // 템플릿 테스트 전송
  const handleTemplateTest = async () => {
    const templateType = selectedTemplate()
    if (!templateType) return

    setIsTemplateTesting(true)
    setTelegramTestResult(null)

    try {
      const result = await testTelegramTemplate({ template_type: templateType })
      setTelegramTestResult(result)
    } catch (err) {
      setTelegramTestResult({
        success: false,
        message: '템플릿 테스트 전송에 실패했습니다.'
      })
    } finally {
      setIsTemplateTesting(false)
    }
  }

  // 모든 템플릿 테스트
  const handleAllTemplatesTest = async () => {
    setIsTemplateTesting(true)
    setTelegramTestResult(null)

    try {
      const result = await testAllTelegramTemplates()
      setTelegramTestResult(result)
    } catch (err) {
      setTelegramTestResult({
        success: false,
        message: '템플릿 테스트에 실패했습니다.'
      })
    } finally {
      setIsTemplateTesting(false)
    }
  }

  const handleSave = () => {
    setIsSaving(true)
    // TODO: 백엔드 설정 저장 API 구현 시 연동
    saveTimeout = setTimeout(() => {
      setIsSaving(false)
      // 로컬 스토리지에 설정 저장 (임시)
      localStorage.setItem('trader_settings', JSON.stringify({
        notifications: notifications(),
        isDarkMode: isDarkMode(),
      }))
    }, 500)
  }

  return (
    <div class="space-y-6 max-w-4xl">
      {/* API 키 관리 (통합 섹션) */}
      <Card>
        <CardHeader>
          <h3 class="text-lg font-semibold text-[var(--color-text)] flex items-center gap-2">
            <Key class="w-5 h-5" />
            API 키 관리
          </h3>
        </CardHeader>
        <CardContent>

        {/* 탭 네비게이션 */}
        <div class="flex gap-2 mb-6 border-b border-[var(--color-surface-light)]">
          <button
            onClick={() => setActiveApiTab('exchange')}
            class={`flex items-center gap-2 px-4 py-3 text-sm font-medium transition-colors border-b-2 -mb-px ${
              activeApiTab() === 'exchange'
                ? 'text-[var(--color-primary)] border-[var(--color-primary)]'
                : 'text-[var(--color-text-muted)] border-transparent hover:text-[var(--color-text)]'
            }`}
          >
            <Building2 class="w-4 h-4" />
            거래소
          </button>
          <button
            onClick={() => setActiveApiTab('notification')}
            class={`flex items-center gap-2 px-4 py-3 text-sm font-medium transition-colors border-b-2 -mb-px ${
              activeApiTab() === 'notification'
                ? 'text-[var(--color-primary)] border-[var(--color-primary)]'
                : 'text-[var(--color-text-muted)] border-transparent hover:text-[var(--color-text)]'
            }`}
          >
            <BellRing class="w-4 h-4" />
            알림 서비스
            <Show when={(notificationServices() || []).length > 0}>
              <span class="w-2 h-2 rounded-full bg-green-500" />
            </Show>
          </button>
        </div>

        {/* 거래소 API 탭 내용 */}
        <Show when={activeApiTab() === 'exchange'}>
          <div class="flex items-center justify-between mb-4">
            <p class="text-sm text-[var(--color-text-muted)]">
              거래소 API 키를 등록하여 자동 매매를 활성화하세요.
            </p>
            <button
              onClick={() => setShowCredentialForm(true)}
              class="px-4 py-2 bg-[var(--color-primary)] text-white rounded-lg text-sm font-medium hover:bg-[var(--color-primary)]/90 transition-colors flex items-center gap-2"
            >
              <Plus class="w-4 h-4" />
              API 키 추가
            </button>
          </div>

          {/* 활성 계정 선택 */}
          <Show when={(credentials() || []).length > 0}>
            <div class="mb-6 p-4 bg-[var(--color-surface-light)] rounded-lg border border-[var(--color-primary)]/30">
              <div class="flex items-center justify-between">
                <div class="flex items-center gap-3">
                  <div class="w-10 h-10 rounded-full bg-[var(--color-primary)]/20 flex items-center justify-center">
                    <Building2 class="w-5 h-5 text-[var(--color-primary)]" />
                  </div>
                  <div>
                    <div class="text-sm font-medium text-[var(--color-text)]">활성 계정</div>
                    <div class="text-xs text-[var(--color-text-muted)]">
                      대시보드에 표시될 자산 정보의 계정을 선택합니다
                    </div>
                  </div>
                </div>
                <div class="flex items-center gap-2">
                  <select
                    value={activeAccount()?.credential_id || ''}
                    onChange={(e) => handleSetActiveAccount(e.currentTarget.value || null)}
                    disabled={isSettingActiveAccount()}
                    class="px-4 py-2 rounded-lg bg-[var(--color-surface)] border border-[var(--color-surface-light)] text-[var(--color-text)] focus:outline-none focus:border-[var(--color-primary)] min-w-[200px] disabled:opacity-50"
                  >
                    <option value="">계정 선택 안함</option>
                    {/* 데이터 제공자(krx)는 거래소가 아니므로 활성 계정 선택에서 제외 */}
                    <For each={(credentials() || []).filter(c => c.exchange_id !== 'krx')}>
                      {(cred) => (
                        <option value={cred.id}>
                          {cred.display_name} ({cred.exchange_id}){cred.is_testnet ? ' [모의투자]' : ''}
                        </option>
                      )}
                    </For>
                  </select>
                  <Show when={isSettingActiveAccount()}>
                    <div class="w-5 h-5 border-2 border-[var(--color-primary)] border-t-transparent rounded-full animate-spin" />
                  </Show>
                </div>
              </div>

              {/* 현재 선택된 활성 계정 정보 표시 */}
              <Show when={activeAccount()?.credential_id}>
                <div class="mt-3 pt-3 border-t border-[var(--color-surface)]">
                  <div class="flex items-center gap-2">
                    <div class="w-2 h-2 rounded-full bg-green-500 animate-pulse" />
                    <span class="text-sm text-[var(--color-text)]">
                      {activeAccount()?.display_name}
                    </span>
                    <Show when={activeAccount()?.is_testnet}>
                      <span class="px-2 py-0.5 text-xs rounded bg-yellow-500/20 text-yellow-500">
                        모의투자
                      </span>
                    </Show>
                    <span class="text-xs text-[var(--color-text-muted)]">
                      ({activeAccount()?.exchange_id})
                    </span>
                  </div>
                </div>
              </Show>
            </div>
          </Show>

        {/* 등록된 자격증명 목록 */}
        <Show
          when={(credentials() || []).length > 0}
          fallback={
            <div class="text-center py-8 text-[var(--color-text-muted)]">
              <Key class="w-12 h-12 mx-auto mb-3 opacity-30" />
              <p>등록된 API 키가 없습니다.</p>
              <p class="text-sm mt-2">거래소 API 키를 추가하여 자동 매매를 활성화하세요.</p>
            </div>
          }
        >
          {/* 거래소 계정 섹션 */}
          <Show when={(credentials() || []).filter(c => !c.is_data_provider).length > 0}>
            <div class="mb-6">
              <h4 class="text-sm font-semibold text-[var(--color-text)] mb-3 flex items-center gap-2">
                <span>🏦</span> 거래소 계정
                <span class="text-xs font-normal text-[var(--color-text-muted)]">
                  (활성 계정으로 선택 가능)
                </span>
              </h4>
              <div class="space-y-3">
                <For each={(credentials() || []).filter(c => !c.is_data_provider)}>
                  {(cred) => (
                    <div class="flex items-center justify-between p-4 bg-[var(--color-surface-light)] rounded-lg">
                      <div class="flex items-center gap-4">
                        <div
                          class={`w-3 h-3 rounded-full ${
                            cred.is_active ? 'bg-green-500' : 'bg-gray-500'
                          }`}
                        />
                        <div>
                          <div class="flex items-center gap-2">
                            <span class="font-medium text-[var(--color-text)]">{cred.display_name}</span>
                            <Show when={cred.is_testnet}>
                              <span class="px-2 py-0.5 text-xs rounded bg-yellow-500/20 text-yellow-500">
                                모의투자
                              </span>
                            </Show>
                            <Show when={cred.exchange_id === 'mock'}>
                              <span class="px-2 py-0.5 text-xs rounded bg-purple-500/20 text-purple-400">
                                🧪 Mock
                              </span>
                            </Show>
                          </div>
                          <div class="text-sm text-[var(--color-text-muted)]">
                            {cred.exchange_id}
                            <Show when={cred.masked_api_key}>
                              {' '}· API: {cred.masked_api_key}
                            </Show>
                          </div>
                          <div class="text-xs text-[var(--color-text-muted)]">
                            등록: {new Date(cred.created_at).toLocaleDateString()}
                            {cred.last_tested_at && ` · 마지막 테스트: ${new Date(cred.last_tested_at).toLocaleDateString()}`}
                          </div>
                        </div>
                      </div>
                      <div class="flex gap-2">
                        <button
                          onClick={() => handleExistingCredentialTest(cred.id)}
                          class="px-3 py-1 text-sm text-blue-500 hover:text-blue-400 transition-colors flex items-center gap-1"
                        >
                          <RefreshCw class="w-4 h-4" />
                          테스트
                        </button>
                        <button
                          onClick={() => handleCredentialDelete(cred.id)}
                          disabled={deletingCredentialId() === cred.id}
                          class="px-3 py-1 text-sm text-red-500 hover:text-red-400 transition-colors flex items-center gap-1 disabled:opacity-50"
                        >
                          <Show when={deletingCredentialId() === cred.id} fallback={<Trash2 class="w-4 h-4" />}>
                            <div class="w-4 h-4 border-2 border-red-500 border-t-transparent rounded-full animate-spin" />
                          </Show>
                          삭제
                        </button>
                      </div>
                    </div>
                  )}
                </For>
              </div>
            </div>
          </Show>

          {/* 데이터 제공자 섹션 */}
          <Show when={(credentials() || []).filter(c => c.is_data_provider).length > 0}>
            <div class="mb-6">
              <h4 class="text-sm font-semibold text-[var(--color-text)] mb-3 flex items-center gap-2">
                <span>📊</span> 데이터 제공자
                <span class="text-xs font-normal text-[var(--color-text-muted)]">
                  (시세 데이터 전용 - 활성 계정으로 선택 불가)
                </span>
              </h4>
              <div class="space-y-3">
                <For each={(credentials() || []).filter(c => c.is_data_provider)}>
                  {(cred) => (
                    <div class="flex items-center justify-between p-4 bg-[var(--color-surface-light)] rounded-lg border border-[var(--color-surface-light)]">
                      <div class="flex items-center gap-4">
                        <div class="w-3 h-3 rounded-full bg-blue-500" />
                        <div>
                          <div class="flex items-center gap-2">
                            <span class="font-medium text-[var(--color-text)]">{cred.display_name}</span>
                            <span class="px-2 py-0.5 text-xs rounded bg-blue-500/20 text-blue-400">
                              데이터 전용
                            </span>
                          </div>
                          <div class="text-sm text-[var(--color-text-muted)]">
                            {cred.exchange_id}
                            <Show when={cred.masked_api_key}>
                              {' '}· API: {cred.masked_api_key}
                            </Show>
                          </div>
                          <div class="text-xs text-[var(--color-text-muted)]">
                            등록: {new Date(cred.created_at).toLocaleDateString()}
                            {cred.last_tested_at && ` · 마지막 테스트: ${new Date(cred.last_tested_at).toLocaleDateString()}`}
                          </div>
                        </div>
                      </div>
                      <div class="flex gap-2">
                        <button
                          onClick={() => handleExistingCredentialTest(cred.id)}
                          class="px-3 py-1 text-sm text-blue-500 hover:text-blue-400 transition-colors flex items-center gap-1"
                        >
                          <RefreshCw class="w-4 h-4" />
                          테스트
                        </button>
                        <button
                          onClick={() => handleCredentialDelete(cred.id)}
                          disabled={deletingCredentialId() === cred.id}
                          class="px-3 py-1 text-sm text-red-500 hover:text-red-400 transition-colors flex items-center gap-1 disabled:opacity-50"
                        >
                          <Show when={deletingCredentialId() === cred.id} fallback={<Trash2 class="w-4 h-4" />}>
                            <div class="w-4 h-4 border-2 border-red-500 border-t-transparent rounded-full animate-spin" />
                          </Show>
                          삭제
                        </button>
                      </div>
                    </div>
                  )}
                </For>
              </div>
            </div>
          </Show>
        </Show>

        {/* 새 자격증명 추가 폼 */}
        <Show when={showCredentialForm()}>
          <div class="border-t border-[var(--color-surface-light)] pt-6 mt-4">
            <h4 class="text-sm font-semibold text-[var(--color-text)] mb-4">새 API 키 등록</h4>

            {/* 거래소 선택 */}
            <div class="mb-4">
              <label class="block text-sm text-[var(--color-text-muted)] mb-1">거래소 선택</label>
              <select
                onChange={(e) => handleExchangeSelect(e.currentTarget.value)}
                class="w-full px-4 py-2 rounded-lg bg-[var(--color-surface-light)] border border-[var(--color-surface-light)] text-[var(--color-text)] focus:outline-none focus:border-[var(--color-primary)]"
              >
                <option value="">거래소를 선택하세요...</option>
                <For each={exchanges()}>
                  {(exchange) => (
                    <option value={exchange.exchange_id}>{exchange.display_name}</option>
                  )}
                </For>
              </select>
            </div>

            {/* 선택된 거래소의 필드들 */}
            <Show when={selectedExchange()}>
              <div class="space-y-4">
                {/* 거래소 설명 */}
                <Show when={selectedExchange()!.description}>
                  <div class="p-3 rounded-lg bg-blue-500/10 border border-blue-500/20">
                    <p class="text-sm text-blue-400">{selectedExchange()!.description}</p>
                    <Show when={selectedExchange()!.docs_url}>
                      <a
                        href={selectedExchange()!.docs_url}
                        target="_blank"
                        rel="noopener noreferrer"
                        class="text-xs text-blue-500 hover:underline mt-1 inline-block"
                      >
                        API 문서 보기 →
                      </a>
                    </Show>
                  </div>
                </Show>

                {/* 표시 이름 */}
                <div>
                  <label class="block text-sm text-[var(--color-text-muted)] mb-1">
                    계정 이름 <span class="text-red-500">*</span>
                  </label>
                  <input
                    type="text"
                    value={credentialDisplayName()}
                    onInput={(e) => setCredentialDisplayName(e.currentTarget.value)}
                    class="w-full px-4 py-2 rounded-lg bg-[var(--color-surface-light)] border border-[var(--color-surface-light)] text-[var(--color-text)] focus:outline-none focus:border-[var(--color-primary)]"
                    placeholder={selectedExchange()!.exchange_id === 'mock'
                      ? "예: 전략A 테스트, 모멘텀 검증용"
                      : "예: 메인 계정, 테스트 계정"
                    }
                  />
                  <Show when={selectedExchange()!.exchange_id === 'mock'}>
                    <p class="text-xs text-[var(--color-text-muted)] mt-1">
                      여러 Mock 계정을 등록하려면 고유한 이름을 사용하세요.
                    </p>
                  </Show>
                </div>

                {/* 필수 필드 */}
                <For each={selectedExchange()!.required_fields}>
                  {(field) => (
                    <div>
                      <label class="block text-sm text-[var(--color-text-muted)] mb-1">
                        {field.label} <span class="text-red-500">*</span>
                      </label>
                      <input
                        type={field.field_type === 'password' ? 'password' : 'text'}
                        value={credentialFields()[field.name] || ''}
                        onInput={(e) => updateField(field.name, e.currentTarget.value)}
                        class="w-full px-4 py-2 rounded-lg bg-[var(--color-surface-light)] border border-[var(--color-surface-light)] text-[var(--color-text)] focus:outline-none focus:border-[var(--color-primary)]"
                        placeholder={field.placeholder || ''}
                      />
                    </div>
                  )}
                </For>

                {/* 선택 필드 */}
                <Show when={selectedExchange()!.optional_fields.length > 0}>
                  <div class="pt-2 border-t border-[var(--color-surface-light)]">
                    <p class="text-xs text-[var(--color-text-muted)] mb-3">선택 항목</p>
                    <For each={selectedExchange()!.optional_fields}>
                      {(field) => (
                        <div class="mb-3">
                          <label class="block text-sm text-[var(--color-text-muted)] mb-1">{field.label}</label>
                          {/* select 타입 필드 */}
                          <Show when={field.field_type === 'select'}>
                            <select
                              value={credentialFields()[field.name] || field.placeholder || ''}
                              onChange={(e) => updateField(field.name, e.currentTarget.value)}
                              class="w-full px-4 py-2 rounded-lg bg-[var(--color-surface-light)] border border-[var(--color-surface-light)] text-[var(--color-text)] focus:outline-none focus:border-[var(--color-primary)]"
                            >
                              {/* Mock 거래소의 market_type 옵션 */}
                              <Show when={field.name === 'market_type'}>
                                <option value="stock_kr">국내 주식 (stock_kr)</option>
                                <option value="stock_us">미국 주식 (stock_us)</option>
                                <option value="crypto">암호화폐 (crypto)</option>
                              </Show>
                            </select>
                          </Show>
                          {/* number 타입 필드 */}
                          <Show when={field.field_type === 'number'}>
                            <input
                              type="number"
                              step={field.name === 'commission_rate' ? '0.001' : '1'}
                              value={credentialFields()[field.name] || field.placeholder || ''}
                              onInput={(e) => updateField(field.name, e.currentTarget.value)}
                              class="w-full px-4 py-2 rounded-lg bg-[var(--color-surface-light)] border border-[var(--color-surface-light)] text-[var(--color-text)] focus:outline-none focus:border-[var(--color-primary)]"
                              placeholder={field.placeholder || ''}
                            />
                          </Show>
                          {/* text/password 타입 필드 */}
                          <Show when={field.field_type !== 'select' && field.field_type !== 'number'}>
                            <input
                              type={field.field_type === 'password' ? 'password' : 'text'}
                              value={credentialFields()[field.name] || ''}
                              onInput={(e) => updateField(field.name, e.currentTarget.value)}
                              class="w-full px-4 py-2 rounded-lg bg-[var(--color-surface-light)] border border-[var(--color-surface-light)] text-[var(--color-text)] focus:outline-none focus:border-[var(--color-primary)]"
                              placeholder={field.placeholder || ''}
                            />
                          </Show>
                          {/* 도움말 텍스트 */}
                          <Show when={field.help_text}>
                            <p class="text-xs text-[var(--color-text-muted)] mt-1">{field.help_text}</p>
                          </Show>
                        </div>
                      )}
                    </For>
                  </div>
                </Show>

                {/* 모의투자/테스트넷 체크박스 */}
                <Show when={selectedExchange()?.supports_testnet}>
                  <div class="pt-3 border-t border-[var(--color-surface-light)]">
                    <label class="flex items-center gap-3 cursor-pointer">
                      <input
                        type="checkbox"
                        checked={isTestnet()}
                        onChange={(e) => setIsTestnet(e.currentTarget.checked)}
                        class="w-5 h-5 rounded border-[var(--color-surface-light)] bg-[var(--color-surface-light)] text-[var(--color-primary)] focus:ring-[var(--color-primary)] cursor-pointer"
                      />
                      <div>
                        <div class="text-[var(--color-text)] font-medium">
                          {selectedExchange()?.market_type === 'crypto' ? '테스트넷 API' : '모의투자 계좌'}
                        </div>
                        <div class="text-sm text-[var(--color-text-muted)]">
                          {selectedExchange()?.market_type === 'crypto'
                            ? '실제 자산을 사용하지 않는 테스트 환경입니다.'
                            : '모의투자 계좌의 API 키입니다. 실제 주문이 체결되지 않습니다.'
                          }
                        </div>
                      </div>
                    </label>
                  </div>
                </Show>

                {/* 테스트 결과 */}
                <Show when={credentialTestResult()}>
                  <div
                    class={`p-3 rounded-lg flex items-center gap-2 ${
                      credentialTestResult()!.success
                        ? 'bg-green-500/20 text-green-400'
                        : 'bg-red-500/20 text-red-400'
                    }`}
                  >
                    <Show when={credentialTestResult()!.success} fallback={<XCircle class="w-5 h-5" />}>
                      <CheckCircle class="w-5 h-5" />
                    </Show>
                    <span>{credentialTestResult()!.message}</span>
                  </div>
                </Show>

                {/* 버튼들 */}
                <div class="flex gap-3 pt-2">
                  <button
                    onClick={handleCredentialTest}
                    disabled={isCredentialTesting()}
                    class="flex-1 px-4 py-2 bg-[var(--color-surface-light)] text-[var(--color-text)] rounded-lg font-medium hover:bg-[var(--color-surface-light)]/80 transition-colors disabled:opacity-50 flex items-center justify-center gap-2"
                  >
                    <Show when={isCredentialTesting()} fallback={<TestTube class="w-4 h-4" />}>
                      <div class="w-4 h-4 border-2 border-current border-t-transparent rounded-full animate-spin" />
                    </Show>
                    연결 테스트
                  </button>
                  <button
                    onClick={handleCredentialSave}
                    disabled={isCredentialSaving() || !credentialTestResult()?.success}
                    class="flex-1 px-4 py-2 bg-[var(--color-primary)] text-white rounded-lg font-medium hover:bg-[var(--color-primary)]/90 transition-colors disabled:opacity-50 disabled:cursor-not-allowed flex items-center justify-center gap-2"
                  >
                    <Show when={isCredentialSaving()} fallback={<Save class="w-4 h-4" />}>
                      <div class="w-4 h-4 border-2 border-white border-t-transparent rounded-full animate-spin" />
                    </Show>
                    저장
                  </button>
                  <button
                    onClick={() => {
                      setShowCredentialForm(false)
                      setSelectedExchange(null)
                      setCredentialFields({})
                      setIsTestnet(false)
                      setCredentialTestResult(null)
                    }}
                    class="px-4 py-2 text-[var(--color-text-muted)] hover:text-[var(--color-text)] transition-colors"
                  >
                    취소
                  </button>
                </div>
              </div>
            </Show>
          </div>
        </Show>
        </Show>

        {/* 알림 서비스 탭 내용 */}
        <Show when={activeApiTab() === 'notification'}>
          <div class="flex items-center justify-between mb-4">
            <p class="text-sm text-[var(--color-text-muted)]">
              알림 서비스를 등록하여 거래 알림을 받으세요.
            </p>
            <button
              onClick={() => setShowNotificationForm(true)}
              class="px-4 py-2 bg-[var(--color-primary)] text-white rounded-lg text-sm font-medium hover:bg-[var(--color-primary)]/90 transition-colors flex items-center gap-2"
            >
              <Plus class="w-4 h-4" />
              알림 서비스 추가
            </button>
          </div>

          {/* 등록된 알림 서비스 목록 */}
          <Show
            when={(notificationServices() || []).length > 0}
            fallback={
              <div class="text-center py-8 text-[var(--color-text-muted)]">
                <BellRing class="w-12 h-12 mx-auto mb-3 opacity-30" />
                <p>등록된 알림 서비스가 없습니다.</p>
                <p class="text-sm mt-2">알림 서비스를 추가하여 거래 알림을 받으세요.</p>
              </div>
            }
          >
            <div class="space-y-3 mb-6">
              <For each={notificationServices()}>
                {(service) => {
                  const provider = NOTIFICATION_PROVIDERS.find(p => p.id === service.provider_id)
                  return (
                    <div class="flex items-center justify-between p-4 bg-[var(--color-surface-light)] rounded-lg">
                      <div class="flex items-center gap-4">
                        <div
                          class={`w-3 h-3 rounded-full ${
                            service.is_active ? 'bg-green-500' : 'bg-gray-500'
                          }`}
                        />
                        <div class="flex items-center gap-3">
                          <span class="text-2xl">{provider?.icon || '🔔'}</span>
                          <div>
                            <div class="flex items-center gap-2">
                              <span class="font-medium text-[var(--color-text)]">{service.display_name}</span>
                              <span class="text-xs px-1.5 py-0.5 rounded bg-[var(--color-primary)]/20 text-[var(--color-primary)]">
                                {provider?.name || service.provider_id}
                              </span>
                            </div>
                            <div class="text-sm text-[var(--color-text-muted)]">
                              {service.masked_info}
                            </div>
                            <div class="text-xs text-[var(--color-text-muted)]">
                              등록: {new Date(service.created_at).toLocaleDateString()}
                              {service.last_tested_at && ` · 마지막 테스트: ${new Date(service.last_tested_at).toLocaleDateString()}`}
                            </div>
                          </div>
                        </div>
                      </div>
                      <div class="flex gap-2">
                        <button
                          onClick={() => handleExistingNotificationTest(service.id, service.provider_id)}
                          class="px-3 py-1 text-sm text-blue-500 hover:text-blue-400 transition-colors flex items-center gap-1"
                        >
                          <TestTube class="w-4 h-4" />
                          테스트
                        </button>
                        <button
                          onClick={() => handleNotificationDelete(service.id, service.provider_id)}
                          disabled={deletingNotificationId() === service.id}
                          class="px-3 py-1 text-sm text-red-500 hover:text-red-400 transition-colors flex items-center gap-1 disabled:opacity-50"
                        >
                          <Show when={deletingNotificationId() === service.id} fallback={<Trash2 class="w-4 h-4" />}>
                            <div class="w-4 h-4 border-2 border-red-500 border-t-transparent rounded-full animate-spin" />
                          </Show>
                          삭제
                        </button>
                      </div>
                    </div>
                  )
                }}
              </For>
            </div>

            {/* 템플릿 테스트 섹션 */}
            <div class="pt-4 border-t border-[var(--color-surface-light)]">
              <h4 class="text-sm font-semibold text-[var(--color-text)] mb-3">
                알림 템플릿 테스트
              </h4>

              <div class="flex gap-3 mb-3">
                <select
                  value={selectedTemplate()}
                  onChange={(e) => setSelectedTemplate(e.currentTarget.value)}
                  class="flex-1 px-4 py-2 rounded-lg bg-[var(--color-surface-light)] border border-[var(--color-surface-light)] text-[var(--color-text)] focus:outline-none focus:border-[var(--color-primary)]"
                >
                  <option value="">템플릿 선택...</option>
                  <For each={templates()}>
                    {(template) => (
                      <option value={template.id}>
                        {template.name} ({template.priority})
                      </option>
                    )}
                  </For>
                </select>

                <button
                  onClick={handleTemplateTest}
                  disabled={isTemplateTesting() || !selectedTemplate()}
                  class="px-4 py-2 bg-[var(--color-primary)] text-white rounded-lg font-medium hover:bg-[var(--color-primary)]/90 transition-colors disabled:opacity-50 disabled:cursor-not-allowed flex items-center gap-2"
                >
                  <Show when={isTemplateTesting()} fallback={<Send class="w-4 h-4" />}>
                    <div class="w-4 h-4 border-2 border-white border-t-transparent rounded-full animate-spin" />
                  </Show>
                  전송
                </button>
              </div>

              <button
                onClick={handleAllTemplatesTest}
                disabled={isTemplateTesting()}
                class="w-full px-4 py-2 bg-[var(--color-surface-light)] text-[var(--color-text)] rounded-lg font-medium hover:bg-[var(--color-surface-light)]/80 transition-colors disabled:opacity-50 disabled:cursor-not-allowed flex items-center justify-center gap-2"
              >
                <Show when={isTemplateTesting()} fallback={<Play class="w-4 h-4" />}>
                  <div class="w-4 h-4 border-2 border-current border-t-transparent rounded-full animate-spin" />
                </Show>
                모든 템플릿 테스트 전송
              </button>

              <Show when={templates()?.length}>
                <div class="mt-4 space-y-2">
                  <p class="text-xs text-[var(--color-text-muted)]">사용 가능한 템플릿:</p>
                  <div class="grid grid-cols-2 gap-2">
                    <For each={templates()}>
                      {(template) => (
                        <div class="text-xs p-2 rounded bg-[var(--color-surface-light)]">
                          <div class="font-medium text-[var(--color-text)]">{template.name}</div>
                          <div class="text-[var(--color-text-muted)]">{template.description}</div>
                        </div>
                      )}
                    </For>
                  </div>
                </div>
              </Show>
            </div>
          </Show>

          {/* 새 알림 서비스 추가 폼 */}
          <Show when={showNotificationForm()}>
            <div class="border-t border-[var(--color-surface-light)] pt-6 mt-4">
              <h4 class="text-sm font-semibold text-[var(--color-text)] mb-4">새 알림 서비스 등록</h4>

              {/* 프로바이더 선택 */}
              <div class="mb-4">
                <label class="block text-sm text-[var(--color-text-muted)] mb-1">알림 서비스 선택</label>
                <select
                  onChange={(e) => handleProviderSelect(e.currentTarget.value)}
                  class="w-full px-4 py-2 rounded-lg bg-[var(--color-surface-light)] border border-[var(--color-surface-light)] text-[var(--color-text)] focus:outline-none focus:border-[var(--color-primary)]"
                >
                  <option value="">알림 서비스를 선택하세요...</option>
                  <For each={NOTIFICATION_PROVIDERS}>
                    {(provider) => (
                      <option value={provider.id}>{provider.icon} {provider.name}</option>
                    )}
                  </For>
                </select>
              </div>

              {/* 선택된 프로바이더의 필드들 */}
              <Show when={selectedProvider()}>
                <div class="space-y-4">
                  <p class="text-sm text-[var(--color-text-muted)]">
                    {selectedProvider()!.description}
                  </p>

                  {/* 표시 이름 */}
                  <div>
                    <label class="block text-sm text-[var(--color-text-muted)] mb-1">표시 이름</label>
                    <input
                      type="text"
                      value={notificationDisplayName()}
                      onInput={(e) => setNotificationDisplayName(e.currentTarget.value)}
                      class="w-full px-4 py-2 rounded-lg bg-[var(--color-surface-light)] border border-[var(--color-surface-light)] text-[var(--color-text)] focus:outline-none focus:border-[var(--color-primary)]"
                      placeholder="예: 메인 알림, 긴급 알림"
                    />
                  </div>

                  {/* 동적 필드 */}
                  <For each={selectedProvider()!.fields}>
                    {(field) => (
                      <div>
                        <label class="block text-sm text-[var(--color-text-muted)] mb-1">
                          {field.label} <span class="text-red-500">*</span>
                        </label>
                        <input
                          type={field.type}
                          value={notificationFields()[field.name] || ''}
                          onInput={(e) => updateNotificationField(field.name, e.currentTarget.value)}
                          class="w-full px-4 py-2 rounded-lg bg-[var(--color-surface-light)] border border-[var(--color-surface-light)] text-[var(--color-text)] focus:outline-none focus:border-[var(--color-primary)]"
                          placeholder={field.placeholder}
                        />
                        <Show when={field.helpText}>
                          <p class="text-xs text-[var(--color-text-muted)] mt-1">{field.helpText}</p>
                        </Show>
                      </div>
                    )}
                  </For>

                  {/* 테스트 결과 */}
                  <Show when={notificationTestResult()}>
                    <div
                      class={`p-3 rounded-lg flex items-center gap-2 ${
                        notificationTestResult()!.success
                          ? 'bg-green-500/20 text-green-400'
                          : 'bg-red-500/20 text-red-400'
                      }`}
                    >
                      <Show when={notificationTestResult()!.success} fallback={<XCircle class="w-5 h-5" />}>
                        <CheckCircle class="w-5 h-5" />
                      </Show>
                      <span>{notificationTestResult()!.message}</span>
                    </div>
                  </Show>

                  {/* 버튼들 */}
                  <div class="flex gap-3 pt-2">
                    <button
                      onClick={handleNotificationTest}
                      disabled={isNotificationTesting()}
                      class="flex-1 px-4 py-2 bg-[var(--color-surface-light)] text-[var(--color-text)] rounded-lg font-medium hover:bg-[var(--color-surface-light)]/80 transition-colors disabled:opacity-50 flex items-center justify-center gap-2"
                    >
                      <Show when={isNotificationTesting()} fallback={<TestTube class="w-4 h-4" />}>
                        <div class="w-4 h-4 border-2 border-current border-t-transparent rounded-full animate-spin" />
                      </Show>
                      연결 테스트
                    </button>
                    <button
                      onClick={handleNotificationSave}
                      disabled={isNotificationSaving() || !notificationTestResult()?.success}
                      class="flex-1 px-4 py-2 bg-[var(--color-primary)] text-white rounded-lg font-medium hover:bg-[var(--color-primary)]/90 transition-colors disabled:opacity-50 disabled:cursor-not-allowed flex items-center justify-center gap-2"
                    >
                      <Show when={isNotificationSaving()} fallback={<Save class="w-4 h-4" />}>
                        <div class="w-4 h-4 border-2 border-white border-t-transparent rounded-full animate-spin" />
                      </Show>
                      저장
                    </button>
                    <button
                      onClick={() => {
                        setShowNotificationForm(false)
                        setSelectedProvider(null)
                        setNotificationFields({})
                        setNotificationDisplayName('')
                        setNotificationTestResult(null)
                      }}
                      class="px-4 py-2 text-[var(--color-text-muted)] hover:text-[var(--color-text)] transition-colors"
                    >
                      취소
                    </button>
                  </div>
                </div>
              </Show>
            </div>
          </Show>
        </Show>
        </CardContent>
      </Card>

      {/* Notifications */}
      <Card>
        <CardHeader>
          <h3 class="text-lg font-semibold text-[var(--color-text)] flex items-center gap-2">
            <Bell class="w-5 h-5" />
            알림 설정
          </h3>
        </CardHeader>
        <CardContent>
          <div class="space-y-4">
          <label class="flex items-center justify-between">
            <div>
              <div class="text-[var(--color-text)]">거래 실행 알림</div>
              <div class="text-sm text-[var(--color-text-muted)]">
                주문이 체결될 때 알림
              </div>
            </div>
            <input
              type="checkbox"
              checked={notifications().tradeExecution}
              onChange={(e) =>
                setNotifications((prev) => ({
                  ...prev,
                  tradeExecution: e.currentTarget.checked,
                }))
              }
              class="w-5 h-5 rounded border-[var(--color-surface-light)] bg-[var(--color-surface-light)] text-[var(--color-primary)] focus:ring-[var(--color-primary)]"
            />
          </label>

          <label class="flex items-center justify-between">
            <div>
              <div class="text-[var(--color-text)]">가격 알림</div>
              <div class="text-sm text-[var(--color-text-muted)]">
                설정한 가격에 도달할 때 알림
              </div>
            </div>
            <input
              type="checkbox"
              checked={notifications().priceAlerts}
              onChange={(e) =>
                setNotifications((prev) => ({
                  ...prev,
                  priceAlerts: e.currentTarget.checked,
                }))
              }
              class="w-5 h-5 rounded border-[var(--color-surface-light)] bg-[var(--color-surface-light)] text-[var(--color-primary)] focus:ring-[var(--color-primary)]"
            />
          </label>

          <label class="flex items-center justify-between">
            <div>
              <div class="text-[var(--color-text)]">일일 리포트</div>
              <div class="text-sm text-[var(--color-text-muted)]">
                매일 거래 요약 리포트
              </div>
            </div>
            <input
              type="checkbox"
              checked={notifications().dailyReport}
              onChange={(e) =>
                setNotifications((prev) => ({
                  ...prev,
                  dailyReport: e.currentTarget.checked,
                }))
              }
              class="w-5 h-5 rounded border-[var(--color-surface-light)] bg-[var(--color-surface-light)] text-[var(--color-primary)] focus:ring-[var(--color-primary)]"
            />
          </label>

          <label class="flex items-center justify-between">
            <div>
              <div class="text-[var(--color-text)]">오류 알림</div>
              <div class="text-sm text-[var(--color-text-muted)]">
                시스템 오류 발생 시 알림
              </div>
            </div>
            <input
              type="checkbox"
              checked={notifications().errorAlerts}
              onChange={(e) =>
                setNotifications((prev) => ({
                  ...prev,
                  errorAlerts: e.currentTarget.checked,
                }))
              }
              class="w-5 h-5 rounded border-[var(--color-surface-light)] bg-[var(--color-surface-light)] text-[var(--color-primary)] focus:ring-[var(--color-primary)]"
            />
          </label>
          </div>
        </CardContent>
      </Card>

      {/* Appearance */}
      <Card>
        <CardHeader>
          <h3 class="text-lg font-semibold text-[var(--color-text)] flex items-center gap-2">
            <Globe class="w-5 h-5" />
            외관 설정
          </h3>
        </CardHeader>
        <CardContent>
          <div class="flex items-center justify-between">
          <div>
            <div class="text-[var(--color-text)]">다크 모드</div>
            <div class="text-sm text-[var(--color-text-muted)]">
              어두운 테마 사용
            </div>
          </div>
          <button
            class={`relative w-14 h-8 rounded-full transition-colors ${
              isDarkMode() ? 'bg-[var(--color-primary)]' : 'bg-[var(--color-surface-light)]'
            }`}
            onClick={() => setIsDarkMode(!isDarkMode())}
          >
            <div
              class={`absolute top-1 w-6 h-6 rounded-full bg-white flex items-center justify-center transition-transform ${
                isDarkMode() ? 'translate-x-7' : 'translate-x-1'
              }`}
            >
              <Show when={isDarkMode()} fallback={<Sun class="w-4 h-4 text-yellow-500" />}>
                <Moon class="w-4 h-4 text-gray-700" />
              </Show>
            </div>
          </button>
          </div>
        </CardContent>
      </Card>

      {/* Database */}
      <Card>
        <CardHeader>
          <h3 class="text-lg font-semibold text-[var(--color-text)] flex items-center gap-2">
            <Database class="w-5 h-5" />
            데이터 관리
          </h3>
        </CardHeader>
        <CardContent>
          <div class="flex flex-wrap gap-3">
            <Button variant="secondary">데이터 내보내기</Button>
            <Button variant="secondary">거래 내역 다운로드</Button>
            <Button variant="danger">캐시 초기화</Button>
          </div>
        </CardContent>
      </Card>

      {/* Save Button */}
      <div class="flex justify-end">
        <Button
          onClick={handleSave}
          disabled={isSaving()}
          className="flex items-center gap-2"
        >
          <Show
            when={isSaving()}
            fallback={<Save class="w-5 h-5" />}
          >
            <div class="w-5 h-5 border-2 border-white border-t-transparent rounded-full animate-spin" />
          </Show>
          {isSaving() ? '저장 중...' : '설정 저장'}
        </Button>
      </div>
    </div>
  )
}

export default Settings
