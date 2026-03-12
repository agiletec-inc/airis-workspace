export type Json =
  | string
  | number
  | boolean
  | null
  | { [key: string]: Json | undefined }
  | Json[]

export type Database = {
  // Allows to automatically instantiate createClient with right options
  // instead of createClient<Database, { PostgrestVersion: 'XX' }>(URL, KEY)
  __InternalSupabase: {
    PostgrestVersion: "14.1"
  }
  public: {
    Tables: {
      _health_check: {
        Row: {
          id: number
          last_check: string | null
        }
        Insert: {
          id?: number
          last_check?: string | null
        }
        Update: {
          id?: number
          last_check?: string | null
        }
        Relationships: []
      }
      admin_users: {
        Row: {
          created_at: string | null
          email: string
          id: string
          permissions: Json | null
          updated_at: string | null
        }
        Insert: {
          created_at?: string | null
          email: string
          id?: string
          permissions?: Json | null
          updated_at?: string | null
        }
        Update: {
          created_at?: string | null
          email?: string
          id?: string
          permissions?: Json | null
          updated_at?: string | null
        }
        Relationships: []
      }
      ai_configurations: {
        Row: {
          created_at: string | null
          id: string
          instructions: string | null
          is_default: boolean | null
          max_tokens: number | null
          model: string
          service_type: string
          temperature: number | null
          tools_config: Json | null
          updated_at: string | null
          voice: string | null
        }
        Insert: {
          created_at?: string | null
          id?: string
          instructions?: string | null
          is_default?: boolean | null
          max_tokens?: number | null
          model: string
          service_type: string
          temperature?: number | null
          tools_config?: Json | null
          updated_at?: string | null
          voice?: string | null
        }
        Update: {
          created_at?: string | null
          id?: string
          instructions?: string | null
          is_default?: boolean | null
          max_tokens?: number | null
          model?: string
          service_type?: string
          temperature?: number | null
          tools_config?: Json | null
          updated_at?: string | null
          voice?: string | null
        }
        Relationships: []
      }
      airis_phone_numbers: {
        Row: {
          created_at: string | null
          id: string
          organization_id: string | null
          phone_number: string
          plan_id: string | null
          plan_type: string | null
          prompt_template_id: string | null
          updated_at: string | null
        }
        Insert: {
          created_at?: string | null
          id?: string
          organization_id?: string | null
          phone_number: string
          plan_id?: string | null
          plan_type?: string | null
          prompt_template_id?: string | null
          updated_at?: string | null
        }
        Update: {
          created_at?: string | null
          id?: string
          organization_id?: string | null
          phone_number?: string
          plan_id?: string | null
          plan_type?: string | null
          prompt_template_id?: string | null
          updated_at?: string | null
        }
        Relationships: [
          {
            foreignKeyName: "airis_phone_numbers_organization_id_fkey"
            columns: ["organization_id"]
            isOneToOne: false
            referencedRelation: "organizations"
            referencedColumns: ["id"]
          },
          {
            foreignKeyName: "airis_phone_numbers_plan_id_fkey"
            columns: ["plan_id"]
            isOneToOne: false
            referencedRelation: "pricing_plans"
            referencedColumns: ["id"]
          },
          {
            foreignKeyName: "airis_phone_numbers_prompt_template_id_fkey"
            columns: ["prompt_template_id"]
            isOneToOne: false
            referencedRelation: "prompt_templates"
            referencedColumns: ["id"]
          },
        ]
      }
      airis_reservations: {
        Row: {
          call_sid: string | null
          created_at: string | null
          customer_name: string
          customer_phone: string
          id: string
          organization_id: string | null
          party_size: number | null
          reservation_date: string
          reservation_time: string
          special_requests: string | null
          status: string | null
          updated_at: string | null
        }
        Insert: {
          call_sid?: string | null
          created_at?: string | null
          customer_name: string
          customer_phone: string
          id?: string
          organization_id?: string | null
          party_size?: number | null
          reservation_date: string
          reservation_time: string
          special_requests?: string | null
          status?: string | null
          updated_at?: string | null
        }
        Update: {
          call_sid?: string | null
          created_at?: string | null
          customer_name?: string
          customer_phone?: string
          id?: string
          organization_id?: string | null
          party_size?: number | null
          reservation_date?: string
          reservation_time?: string
          special_requests?: string | null
          status?: string | null
          updated_at?: string | null
        }
        Relationships: [
          {
            foreignKeyName: "airis_reservations_organization_id_fkey"
            columns: ["organization_id"]
            isOneToOne: false
            referencedRelation: "organizations"
            referencedColumns: ["id"]
          },
        ]
      }
      api_keys: {
        Row: {
          created_at: string | null
          expires_at: string | null
          id: string
          is_active: boolean | null
          key_hash: string
          last_used_at: string | null
          name: string
          organization_id: string | null
          updated_at: string | null
        }
        Insert: {
          created_at?: string | null
          expires_at?: string | null
          id?: string
          is_active?: boolean | null
          key_hash: string
          last_used_at?: string | null
          name: string
          organization_id?: string | null
          updated_at?: string | null
        }
        Update: {
          created_at?: string | null
          expires_at?: string | null
          id?: string
          is_active?: boolean | null
          key_hash?: string
          last_used_at?: string | null
          name?: string
          organization_id?: string | null
          updated_at?: string | null
        }
        Relationships: [
          {
            foreignKeyName: "api_keys_organization_id_fkey"
            columns: ["organization_id"]
            isOneToOne: false
            referencedRelation: "organizations"
            referencedColumns: ["id"]
          },
        ]
      }
      autocall_campaigns: {
        Row: {
          created_at: string | null
          created_by: string | null
          description: string | null
          id: string
          max_concurrent_calls: number | null
          metadata: Json | null
          name: string
          organization_id: string
          retry_attempts: number | null
          schedule: Json | null
          script_id: string | null
          status: string | null
          target_numbers: string[]
          updated_at: string | null
        }
        Insert: {
          created_at?: string | null
          created_by?: string | null
          description?: string | null
          id?: string
          max_concurrent_calls?: number | null
          metadata?: Json | null
          name: string
          organization_id: string
          retry_attempts?: number | null
          schedule?: Json | null
          script_id?: string | null
          status?: string | null
          target_numbers: string[]
          updated_at?: string | null
        }
        Update: {
          created_at?: string | null
          created_by?: string | null
          description?: string | null
          id?: string
          max_concurrent_calls?: number | null
          metadata?: Json | null
          name?: string
          organization_id?: string
          retry_attempts?: number | null
          schedule?: Json | null
          script_id?: string | null
          status?: string | null
          target_numbers?: string[]
          updated_at?: string | null
        }
        Relationships: [
          {
            foreignKeyName: "autocall_campaigns_organization_id_fkey"
            columns: ["organization_id"]
            isOneToOne: false
            referencedRelation: "organizations"
            referencedColumns: ["id"]
          },
          {
            foreignKeyName: "autocall_campaigns_script_id_fkey"
            columns: ["script_id"]
            isOneToOne: false
            referencedRelation: "scripts"
            referencedColumns: ["id"]
          },
        ]
      }
      business_rules: {
        Row: {
          created_at: string | null
          description: string | null
          id: string
          is_active: boolean | null
          rule_category: string
          rule_name: string
          rule_type: string
          rule_value: Json
          updated_at: string | null
        }
        Insert: {
          created_at?: string | null
          description?: string | null
          id?: string
          is_active?: boolean | null
          rule_category: string
          rule_name: string
          rule_type: string
          rule_value: Json
          updated_at?: string | null
        }
        Update: {
          created_at?: string | null
          description?: string | null
          id?: string
          is_active?: boolean | null
          rule_category?: string
          rule_name?: string
          rule_type?: string
          rule_value?: Json
          updated_at?: string | null
        }
        Relationships: []
      }
      call_insights: {
        Row: {
          call_sid: string
          extracted_at: string | null
          id: string
          insights: Json
        }
        Insert: {
          call_sid: string
          extracted_at?: string | null
          id?: string
          insights: Json
        }
        Update: {
          call_sid?: string
          extracted_at?: string | null
          id?: string
          insights?: Json
        }
        Relationships: []
      }
      call_logs: {
        Row: {
          call_sid: string
          created_at: string | null
          event_data: Json | null
          event_type: string | null
          id: string
          timestamp: string | null
        }
        Insert: {
          call_sid: string
          created_at?: string | null
          event_data?: Json | null
          event_type?: string | null
          id?: string
          timestamp?: string | null
        }
        Update: {
          call_sid?: string
          created_at?: string | null
          event_data?: Json | null
          event_type?: string | null
          id?: string
          timestamp?: string | null
        }
        Relationships: []
      }
      call_metrics: {
        Row: {
          analyzed_at: string | null
          audio_latency_ms: number | null
          call_sid: string
          created_at: string | null
          duration_seconds: number | null
          error_count: number | null
          id: string
          metrics: Json
          organization_id: string
          reconnect_count: number | null
          status: string | null
          transcription_accuracy: number | null
          updated_at: string | null
        }
        Insert: {
          analyzed_at?: string | null
          audio_latency_ms?: number | null
          call_sid: string
          created_at?: string | null
          duration_seconds?: number | null
          error_count?: number | null
          id?: string
          metrics: Json
          organization_id: string
          reconnect_count?: number | null
          status?: string | null
          transcription_accuracy?: number | null
          updated_at?: string | null
        }
        Update: {
          analyzed_at?: string | null
          audio_latency_ms?: number | null
          call_sid?: string
          created_at?: string | null
          duration_seconds?: number | null
          error_count?: number | null
          id?: string
          metrics?: Json
          organization_id?: string
          reconnect_count?: number | null
          status?: string | null
          transcription_accuracy?: number | null
          updated_at?: string | null
        }
        Relationships: [
          {
            foreignKeyName: "call_metrics_organization_id_fkey"
            columns: ["organization_id"]
            isOneToOne: false
            referencedRelation: "organizations"
            referencedColumns: ["id"]
          },
        ]
      }
      call_recordings: {
        Row: {
          call_sid: string
          created_at: string | null
          duration: number | null
          id: string
          organization_id: string | null
          recording_sid: string | null
          recording_url: string | null
          storage_bucket: string | null
          storage_metadata: Json | null
          storage_path: string | null
          storage_size: number | null
          updated_at: string | null
        }
        Insert: {
          call_sid: string
          created_at?: string | null
          duration?: number | null
          id?: string
          organization_id?: string | null
          recording_sid?: string | null
          recording_url?: string | null
          storage_bucket?: string | null
          storage_metadata?: Json | null
          storage_path?: string | null
          storage_size?: number | null
          updated_at?: string | null
        }
        Update: {
          call_sid?: string
          created_at?: string | null
          duration?: number | null
          id?: string
          organization_id?: string | null
          recording_sid?: string | null
          recording_url?: string | null
          storage_bucket?: string | null
          storage_metadata?: Json | null
          storage_path?: string | null
          storage_size?: number | null
          updated_at?: string | null
        }
        Relationships: [
          {
            foreignKeyName: "call_recordings_organization_id_fkey"
            columns: ["organization_id"]
            isOneToOne: false
            referencedRelation: "organizations"
            referencedColumns: ["id"]
          },
        ]
      }
      call_summaries: {
        Row: {
          action_items: Json | null
          call_sid: string
          created_at: string | null
          id: string
          key_points: Json | null
          metadata: Json | null
          organization_id: string | null
          sentiment: string | null
          summary: string | null
          updated_at: string | null
        }
        Insert: {
          action_items?: Json | null
          call_sid: string
          created_at?: string | null
          id?: string
          key_points?: Json | null
          metadata?: Json | null
          organization_id?: string | null
          sentiment?: string | null
          summary?: string | null
          updated_at?: string | null
        }
        Update: {
          action_items?: Json | null
          call_sid?: string
          created_at?: string | null
          id?: string
          key_points?: Json | null
          metadata?: Json | null
          organization_id?: string | null
          sentiment?: string | null
          summary?: string | null
          updated_at?: string | null
        }
        Relationships: [
          {
            foreignKeyName: "call_summaries_organization_id_fkey"
            columns: ["organization_id"]
            isOneToOne: false
            referencedRelation: "organizations"
            referencedColumns: ["id"]
          },
        ]
      }
      call_transcripts: {
        Row: {
          call_sid: string
          confidence: number | null
          created_at: string | null
          id: string
          language: string | null
          metadata: Json | null
          organization_id: string | null
          transcript: string | null
          updated_at: string | null
        }
        Insert: {
          call_sid: string
          confidence?: number | null
          created_at?: string | null
          id?: string
          language?: string | null
          metadata?: Json | null
          organization_id?: string | null
          transcript?: string | null
          updated_at?: string | null
        }
        Update: {
          call_sid?: string
          confidence?: number | null
          created_at?: string | null
          id?: string
          language?: string | null
          metadata?: Json | null
          organization_id?: string | null
          transcript?: string | null
          updated_at?: string | null
        }
        Relationships: [
          {
            foreignKeyName: "call_transcripts_organization_id_fkey"
            columns: ["organization_id"]
            isOneToOne: false
            referencedRelation: "organizations"
            referencedColumns: ["id"]
          },
        ]
      }
      calls: {
        Row: {
          call_sid: string
          created_at: string | null
          direction: string | null
          duration: number | null
          from_number: string | null
          id: string
          metadata: Json | null
          organization_id: string | null
          recording_url: string | null
          status: string | null
          to_number: string | null
          updated_at: string | null
        }
        Insert: {
          call_sid: string
          created_at?: string | null
          direction?: string | null
          duration?: number | null
          from_number?: string | null
          id?: string
          metadata?: Json | null
          organization_id?: string | null
          recording_url?: string | null
          status?: string | null
          to_number?: string | null
          updated_at?: string | null
        }
        Update: {
          call_sid?: string
          created_at?: string | null
          direction?: string | null
          duration?: number | null
          from_number?: string | null
          id?: string
          metadata?: Json | null
          organization_id?: string | null
          recording_url?: string | null
          status?: string | null
          to_number?: string | null
          updated_at?: string | null
        }
        Relationships: [
          {
            foreignKeyName: "calls_organization_id_fkey"
            columns: ["organization_id"]
            isOneToOne: false
            referencedRelation: "organizations"
            referencedColumns: ["id"]
          },
        ]
      }
      campaign_calls: {
        Row: {
          call_sid: string
          campaign_id: string | null
          created_at: string | null
          id: string
          phone_number: string | null
          result: string | null
          status: string | null
          updated_at: string | null
        }
        Insert: {
          call_sid: string
          campaign_id?: string | null
          created_at?: string | null
          id?: string
          phone_number?: string | null
          result?: string | null
          status?: string | null
          updated_at?: string | null
        }
        Update: {
          call_sid?: string
          campaign_id?: string | null
          created_at?: string | null
          id?: string
          phone_number?: string | null
          result?: string | null
          status?: string | null
          updated_at?: string | null
        }
        Relationships: [
          {
            foreignKeyName: "campaign_calls_campaign_id_fkey"
            columns: ["campaign_id"]
            isOneToOne: false
            referencedRelation: "campaigns"
            referencedColumns: ["id"]
          },
        ]
      }
      campaigns: {
        Row: {
          call_script: string | null
          created_at: string | null
          description: string | null
          id: string
          name: string
          organization_id: string | null
          schedule_config: Json | null
          status: string | null
          target_list: Json | null
          updated_at: string | null
        }
        Insert: {
          call_script?: string | null
          created_at?: string | null
          description?: string | null
          id?: string
          name: string
          organization_id?: string | null
          schedule_config?: Json | null
          status?: string | null
          target_list?: Json | null
          updated_at?: string | null
        }
        Update: {
          call_script?: string | null
          created_at?: string | null
          description?: string | null
          id?: string
          name?: string
          organization_id?: string | null
          schedule_config?: Json | null
          status?: string | null
          target_list?: Json | null
          updated_at?: string | null
        }
        Relationships: [
          {
            foreignKeyName: "campaigns_organization_id_fkey"
            columns: ["organization_id"]
            isOneToOne: false
            referencedRelation: "organizations"
            referencedColumns: ["id"]
          },
        ]
      }
      categories: {
        Row: {
          color: string
          created_at: string
          icon: string | null
          id: string
          name: string
          sort_order: number
          updated_at: string
          user_id: string
        }
        Insert: {
          color?: string
          created_at?: string
          icon?: string | null
          id?: string
          name: string
          sort_order?: number
          updated_at?: string
          user_id: string
        }
        Update: {
          color?: string
          created_at?: string
          icon?: string | null
          id?: string
          name?: string
          sort_order?: number
          updated_at?: string
          user_id?: string
        }
        Relationships: []
      }
      cc_customers: {
        Row: {
          company: string | null
          created_at: string | null
          email: string | null
          id: string
          last_contact_at: string | null
          name: string
          notes: string | null
          organization_id: string | null
          phone: string | null
          tags: string[] | null
          updated_at: string | null
        }
        Insert: {
          company?: string | null
          created_at?: string | null
          email?: string | null
          id?: string
          last_contact_at?: string | null
          name: string
          notes?: string | null
          organization_id?: string | null
          phone?: string | null
          tags?: string[] | null
          updated_at?: string | null
        }
        Update: {
          company?: string | null
          created_at?: string | null
          email?: string | null
          id?: string
          last_contact_at?: string | null
          name?: string
          notes?: string | null
          organization_id?: string | null
          phone?: string | null
          tags?: string[] | null
          updated_at?: string | null
        }
        Relationships: [
          {
            foreignKeyName: "cc_customers_organization_id_fkey"
            columns: ["organization_id"]
            isOneToOne: false
            referencedRelation: "organizations"
            referencedColumns: ["id"]
          },
        ]
      }
      chat_integrations: {
        Row: {
          channel: string
          created_at: string | null
          credentials: Json
          display_name: string
          id: string
          last_error: string | null
          last_used_at: string | null
          organization_id: string
          status: string | null
          updated_at: string | null
        }
        Insert: {
          channel: string
          created_at?: string | null
          credentials?: Json
          display_name: string
          id?: string
          last_error?: string | null
          last_used_at?: string | null
          organization_id: string
          status?: string | null
          updated_at?: string | null
        }
        Update: {
          channel?: string
          created_at?: string | null
          credentials?: Json
          display_name?: string
          id?: string
          last_error?: string | null
          last_used_at?: string | null
          organization_id?: string
          status?: string | null
          updated_at?: string | null
        }
        Relationships: [
          {
            foreignKeyName: "chat_integrations_organization_id_fkey"
            columns: ["organization_id"]
            isOneToOne: false
            referencedRelation: "organizations"
            referencedColumns: ["id"]
          },
        ]
      }
      chat_messages: {
        Row: {
          content: string
          created_at: string
          id: string
          role: string
          session_id: string
          tool_invocations: Json | null
          user_id: string
        }
        Insert: {
          content: string
          created_at?: string
          id?: string
          role: string
          session_id: string
          tool_invocations?: Json | null
          user_id: string
        }
        Update: {
          content?: string
          created_at?: string
          id?: string
          role?: string
          session_id?: string
          tool_invocations?: Json | null
          user_id?: string
        }
        Relationships: [
          {
            foreignKeyName: "chat_messages_session_id_fkey"
            columns: ["session_id"]
            isOneToOne: false
            referencedRelation: "chat_sessions"
            referencedColumns: ["id"]
          },
        ]
      }
      chat_sessions: {
        Row: {
          created_at: string
          id: string
          title: string | null
          updated_at: string
          user_id: string
        }
        Insert: {
          created_at?: string
          id?: string
          title?: string | null
          updated_at?: string
          user_id: string
        }
        Update: {
          created_at?: string
          id?: string
          title?: string | null
          updated_at?: string
          user_id?: string
        }
        Relationships: []
      }
      edge_function_metrics: {
        Row: {
          created_at: string
          environment: string | null
          function_name: string
          id: string
          metric_name: string
          metric_unit: string | null
          metric_value: number
          tags: Json | null
          timestamp: string
        }
        Insert: {
          created_at?: string
          environment?: string | null
          function_name: string
          id?: string
          metric_name: string
          metric_unit?: string | null
          metric_value: number
          tags?: Json | null
          timestamp?: string
        }
        Update: {
          created_at?: string
          environment?: string | null
          function_name?: string
          id?: string
          metric_name?: string
          metric_unit?: string | null
          metric_value?: number
          tags?: Json | null
          timestamp?: string
        }
        Relationships: []
      }
      evidence_dictionary: {
        Row: {
          category: string | null
          created_at: string | null
          created_by: string | null
          definition: string | null
          domain: string | null
          id: string
          is_active: boolean | null
          organization_id: string
          reading: string | null
          synonyms: string[] | null
          term: string
          updated_at: string | null
          usage_examples: string[] | null
        }
        Insert: {
          category?: string | null
          created_at?: string | null
          created_by?: string | null
          definition?: string | null
          domain?: string | null
          id?: string
          is_active?: boolean | null
          organization_id: string
          reading?: string | null
          synonyms?: string[] | null
          term: string
          updated_at?: string | null
          usage_examples?: string[] | null
        }
        Update: {
          category?: string | null
          created_at?: string | null
          created_by?: string | null
          definition?: string | null
          domain?: string | null
          id?: string
          is_active?: boolean | null
          organization_id?: string
          reading?: string | null
          synonyms?: string[] | null
          term?: string
          updated_at?: string | null
          usage_examples?: string[] | null
        }
        Relationships: [
          {
            foreignKeyName: "evidence_dictionary_organization_id_fkey"
            columns: ["organization_id"]
            isOneToOne: false
            referencedRelation: "organizations"
            referencedColumns: ["id"]
          },
        ]
      }
      evidence_sessions: {
        Row: {
          created_at: string | null
          created_by: string | null
          description: string | null
          id: string
          metadata: Json | null
          organization_id: string
          participants: string[] | null
          session_name: string
          session_type: string | null
          tags: string[] | null
          updated_at: string | null
        }
        Insert: {
          created_at?: string | null
          created_by?: string | null
          description?: string | null
          id?: string
          metadata?: Json | null
          organization_id: string
          participants?: string[] | null
          session_name: string
          session_type?: string | null
          tags?: string[] | null
          updated_at?: string | null
        }
        Update: {
          created_at?: string | null
          created_by?: string | null
          description?: string | null
          id?: string
          metadata?: Json | null
          organization_id?: string
          participants?: string[] | null
          session_name?: string
          session_type?: string | null
          tags?: string[] | null
          updated_at?: string | null
        }
        Relationships: [
          {
            foreignKeyName: "evidence_sessions_organization_id_fkey"
            columns: ["organization_id"]
            isOneToOne: false
            referencedRelation: "organizations"
            referencedColumns: ["id"]
          },
        ]
      }
      evidence_text_analysis: {
        Row: {
          created_at: string | null
          entities: Json | null
          id: string
          keywords: Json | null
          language_stats: Json | null
          organization_id: string
          sentiment: Json | null
          summary: string | null
          summary_model: string | null
          topics: Json | null
          transcription_id: string | null
          updated_at: string | null
        }
        Insert: {
          created_at?: string | null
          entities?: Json | null
          id?: string
          keywords?: Json | null
          language_stats?: Json | null
          organization_id: string
          sentiment?: Json | null
          summary?: string | null
          summary_model?: string | null
          topics?: Json | null
          transcription_id?: string | null
          updated_at?: string | null
        }
        Update: {
          created_at?: string | null
          entities?: Json | null
          id?: string
          keywords?: Json | null
          language_stats?: Json | null
          organization_id?: string
          sentiment?: Json | null
          summary?: string | null
          summary_model?: string | null
          topics?: Json | null
          transcription_id?: string | null
          updated_at?: string | null
        }
        Relationships: [
          {
            foreignKeyName: "evidence_text_analysis_organization_id_fkey"
            columns: ["organization_id"]
            isOneToOne: false
            referencedRelation: "organizations"
            referencedColumns: ["id"]
          },
          {
            foreignKeyName: "evidence_text_analysis_transcription_id_fkey"
            columns: ["transcription_id"]
            isOneToOne: false
            referencedRelation: "evidence_transcriptions"
            referencedColumns: ["id"]
          },
        ]
      }
      evidence_transcriptions: {
        Row: {
          created_at: string | null
          created_by: string | null
          duration: number | null
          error_message: string | null
          file_name: string
          file_size: number | null
          file_type: string | null
          file_url: string | null
          id: string
          language: string | null
          model: string | null
          organization_id: string
          processing_time_ms: number | null
          segments: Json | null
          status: string | null
          transcription_text: string | null
          updated_at: string | null
        }
        Insert: {
          created_at?: string | null
          created_by?: string | null
          duration?: number | null
          error_message?: string | null
          file_name: string
          file_size?: number | null
          file_type?: string | null
          file_url?: string | null
          id?: string
          language?: string | null
          model?: string | null
          organization_id: string
          processing_time_ms?: number | null
          segments?: Json | null
          status?: string | null
          transcription_text?: string | null
          updated_at?: string | null
        }
        Update: {
          created_at?: string | null
          created_by?: string | null
          duration?: number | null
          error_message?: string | null
          file_name?: string
          file_size?: number | null
          file_type?: string | null
          file_url?: string | null
          id?: string
          language?: string | null
          model?: string | null
          organization_id?: string
          processing_time_ms?: number | null
          segments?: Json | null
          status?: string | null
          transcription_text?: string | null
          updated_at?: string | null
        }
        Relationships: [
          {
            foreignKeyName: "evidence_transcriptions_organization_id_fkey"
            columns: ["organization_id"]
            isOneToOne: false
            referencedRelation: "organizations"
            referencedColumns: ["id"]
          },
        ]
      }
      fax_documents: {
        Row: {
          api_response: Json | null
          api_sent: boolean | null
          api_sent_at: string | null
          confidence_score: number | null
          created_at: string | null
          document_type: string | null
          duration: number | null
          error_message: string | null
          extracted_data: Json | null
          from_number: string
          id: string
          ocr_processed_at: string | null
          ocr_result: Json | null
          ocr_status: string | null
          ocr_text: string | null
          organization_id: string
          pages: number | null
          pdf_url: string
          plan_type: string | null
          processed_at: string | null
          received_at: string | null
          slack_message_ts: string | null
          slack_sent: boolean | null
          slack_sent_at: string | null
          status: string | null
          thumbnail_url: string | null
          to_number: string
          updated_at: string | null
        }
        Insert: {
          api_response?: Json | null
          api_sent?: boolean | null
          api_sent_at?: string | null
          confidence_score?: number | null
          created_at?: string | null
          document_type?: string | null
          duration?: number | null
          error_message?: string | null
          extracted_data?: Json | null
          from_number: string
          id?: string
          ocr_processed_at?: string | null
          ocr_result?: Json | null
          ocr_status?: string | null
          ocr_text?: string | null
          organization_id: string
          pages?: number | null
          pdf_url: string
          plan_type?: string | null
          processed_at?: string | null
          received_at?: string | null
          slack_message_ts?: string | null
          slack_sent?: boolean | null
          slack_sent_at?: string | null
          status?: string | null
          thumbnail_url?: string | null
          to_number: string
          updated_at?: string | null
        }
        Update: {
          api_response?: Json | null
          api_sent?: boolean | null
          api_sent_at?: string | null
          confidence_score?: number | null
          created_at?: string | null
          document_type?: string | null
          duration?: number | null
          error_message?: string | null
          extracted_data?: Json | null
          from_number?: string
          id?: string
          ocr_processed_at?: string | null
          ocr_result?: Json | null
          ocr_status?: string | null
          ocr_text?: string | null
          organization_id?: string
          pages?: number | null
          pdf_url?: string
          plan_type?: string | null
          processed_at?: string | null
          received_at?: string | null
          slack_message_ts?: string | null
          slack_sent?: boolean | null
          slack_sent_at?: string | null
          status?: string | null
          thumbnail_url?: string | null
          to_number?: string
          updated_at?: string | null
        }
        Relationships: [
          {
            foreignKeyName: "fax_documents_organization_id_fkey"
            columns: ["organization_id"]
            isOneToOne: false
            referencedRelation: "organizations"
            referencedColumns: ["id"]
          },
        ]
      }
      fax_notification_settings: {
        Row: {
          created_at: string | null
          email_addresses: string[] | null
          email_enabled: boolean | null
          filter_rules: Json | null
          id: string
          is_active: boolean | null
          line_channel_token: string | null
          line_enabled: boolean | null
          line_user_id: string | null
          notify_on_error: boolean | null
          notify_on_ocr_complete: boolean | null
          notify_on_receive: boolean | null
          organization_id: string
          slack_channel: string | null
          slack_enabled: boolean | null
          slack_webhook_url: string | null
          teams_enabled: boolean | null
          teams_webhook_url: string | null
          updated_at: string | null
        }
        Insert: {
          created_at?: string | null
          email_addresses?: string[] | null
          email_enabled?: boolean | null
          filter_rules?: Json | null
          id?: string
          is_active?: boolean | null
          line_channel_token?: string | null
          line_enabled?: boolean | null
          line_user_id?: string | null
          notify_on_error?: boolean | null
          notify_on_ocr_complete?: boolean | null
          notify_on_receive?: boolean | null
          organization_id: string
          slack_channel?: string | null
          slack_enabled?: boolean | null
          slack_webhook_url?: string | null
          teams_enabled?: boolean | null
          teams_webhook_url?: string | null
          updated_at?: string | null
        }
        Update: {
          created_at?: string | null
          email_addresses?: string[] | null
          email_enabled?: boolean | null
          filter_rules?: Json | null
          id?: string
          is_active?: boolean | null
          line_channel_token?: string | null
          line_enabled?: boolean | null
          line_user_id?: string | null
          notify_on_error?: boolean | null
          notify_on_ocr_complete?: boolean | null
          notify_on_receive?: boolean | null
          organization_id?: string
          slack_channel?: string | null
          slack_enabled?: boolean | null
          slack_webhook_url?: string | null
          teams_enabled?: boolean | null
          teams_webhook_url?: string | null
          updated_at?: string | null
        }
        Relationships: [
          {
            foreignKeyName: "fax_notification_settings_organization_id_fkey"
            columns: ["organization_id"]
            isOneToOne: true
            referencedRelation: "organizations"
            referencedColumns: ["id"]
          },
        ]
      }
      fax_numbers: {
        Row: {
          auto_notify: boolean | null
          auto_ocr: boolean | null
          capabilities: Json | null
          country_code: string | null
          created_at: string | null
          display_name: string | null
          id: string
          is_plan_included: boolean
          last_received_at: string | null
          monthly_cost: number | null
          organization_id: string
          phone_number: string
          service_subscription_id: string | null
          status: string | null
          total_received: number | null
          twilio_sid: string | null
          updated_at: string | null
          webhook_url: string | null
        }
        Insert: {
          auto_notify?: boolean | null
          auto_ocr?: boolean | null
          capabilities?: Json | null
          country_code?: string | null
          created_at?: string | null
          display_name?: string | null
          id?: string
          is_plan_included?: boolean
          last_received_at?: string | null
          monthly_cost?: number | null
          organization_id: string
          phone_number: string
          service_subscription_id?: string | null
          status?: string | null
          total_received?: number | null
          twilio_sid?: string | null
          updated_at?: string | null
          webhook_url?: string | null
        }
        Update: {
          auto_notify?: boolean | null
          auto_ocr?: boolean | null
          capabilities?: Json | null
          country_code?: string | null
          created_at?: string | null
          display_name?: string | null
          id?: string
          is_plan_included?: boolean
          last_received_at?: string | null
          monthly_cost?: number | null
          organization_id?: string
          phone_number?: string
          service_subscription_id?: string | null
          status?: string | null
          total_received?: number | null
          twilio_sid?: string | null
          updated_at?: string | null
          webhook_url?: string | null
        }
        Relationships: [
          {
            foreignKeyName: "fax_numbers_organization_id_fkey"
            columns: ["organization_id"]
            isOneToOne: false
            referencedRelation: "organizations"
            referencedColumns: ["id"]
          },
          {
            foreignKeyName: "fax_numbers_service_subscription_id_fkey"
            columns: ["service_subscription_id"]
            isOneToOne: false
            referencedRelation: "service_subscriptions"
            referencedColumns: ["id"]
          },
        ]
      }
      fax_ocr_integration: {
        Row: {
          auto_export_format: string | null
          created_at: string | null
          fax_auto_ocr: boolean | null
          fax_ocr_priority: string | null
          id: string
          ocr_subscription_id: string | null
          organization_id: string
          updated_at: string | null
          use_separate_ocr_quota: boolean | null
          webhook_url: string | null
        }
        Insert: {
          auto_export_format?: string | null
          created_at?: string | null
          fax_auto_ocr?: boolean | null
          fax_ocr_priority?: string | null
          id?: string
          ocr_subscription_id?: string | null
          organization_id: string
          updated_at?: string | null
          use_separate_ocr_quota?: boolean | null
          webhook_url?: string | null
        }
        Update: {
          auto_export_format?: string | null
          created_at?: string | null
          fax_auto_ocr?: boolean | null
          fax_ocr_priority?: string | null
          id?: string
          ocr_subscription_id?: string | null
          organization_id?: string
          updated_at?: string | null
          use_separate_ocr_quota?: boolean | null
          webhook_url?: string | null
        }
        Relationships: [
          {
            foreignKeyName: "fax_ocr_integration_ocr_subscription_id_fkey"
            columns: ["ocr_subscription_id"]
            isOneToOne: false
            referencedRelation: "ocr_subscriptions"
            referencedColumns: ["id"]
          },
          {
            foreignKeyName: "fax_ocr_integration_organization_id_fkey"
            columns: ["organization_id"]
            isOneToOne: true
            referencedRelation: "organizations"
            referencedColumns: ["id"]
          },
        ]
      }
      fax_processing_queue: {
        Row: {
          attempts: number | null
          completed_at: string | null
          created_at: string | null
          error_message: string | null
          fax_document_id: string
          id: string
          max_attempts: number | null
          payload: Json | null
          priority: number | null
          result: Json | null
          scheduled_at: string | null
          started_at: string | null
          status: string | null
          task_type: string
          updated_at: string | null
        }
        Insert: {
          attempts?: number | null
          completed_at?: string | null
          created_at?: string | null
          error_message?: string | null
          fax_document_id: string
          id?: string
          max_attempts?: number | null
          payload?: Json | null
          priority?: number | null
          result?: Json | null
          scheduled_at?: string | null
          started_at?: string | null
          status?: string | null
          task_type: string
          updated_at?: string | null
        }
        Update: {
          attempts?: number | null
          completed_at?: string | null
          created_at?: string | null
          error_message?: string | null
          fax_document_id?: string
          id?: string
          max_attempts?: number | null
          payload?: Json | null
          priority?: number | null
          result?: Json | null
          scheduled_at?: string | null
          started_at?: string | null
          status?: string | null
          task_type?: string
          updated_at?: string | null
        }
        Relationships: [
          {
            foreignKeyName: "fax_processing_queue_fax_document_id_fkey"
            columns: ["fax_document_id"]
            isOneToOne: false
            referencedRelation: "fax_documents"
            referencedColumns: ["id"]
          },
        ]
      }
      fax_settings: {
        Row: {
          api_integration_enabled: boolean | null
          api_webhook_url: string | null
          auto_archive: boolean | null
          created_at: string | null
          id: string
          ocr_enabled: boolean | null
          ocr_language: string | null
          organization_id: string
          phone_number: string
          plan_type: string
          retention_days: number | null
          settings: Json | null
          slack_channel: string | null
          slack_webhook_url: string | null
          updated_at: string | null
        }
        Insert: {
          api_integration_enabled?: boolean | null
          api_webhook_url?: string | null
          auto_archive?: boolean | null
          created_at?: string | null
          id?: string
          ocr_enabled?: boolean | null
          ocr_language?: string | null
          organization_id: string
          phone_number: string
          plan_type: string
          retention_days?: number | null
          settings?: Json | null
          slack_channel?: string | null
          slack_webhook_url?: string | null
          updated_at?: string | null
        }
        Update: {
          api_integration_enabled?: boolean | null
          api_webhook_url?: string | null
          auto_archive?: boolean | null
          created_at?: string | null
          id?: string
          ocr_enabled?: boolean | null
          ocr_language?: string | null
          organization_id?: string
          phone_number?: string
          plan_type?: string
          retention_days?: number | null
          settings?: Json | null
          slack_channel?: string | null
          slack_webhook_url?: string | null
          updated_at?: string | null
        }
        Relationships: [
          {
            foreignKeyName: "fax_settings_organization_id_fkey"
            columns: ["organization_id"]
            isOneToOne: false
            referencedRelation: "organizations"
            referencedColumns: ["id"]
          },
          {
            foreignKeyName: "fax_settings_phone_number_fkey"
            columns: ["phone_number"]
            isOneToOne: false
            referencedRelation: "phone_numbers"
            referencedColumns: ["phone_number"]
          },
        ]
      }
      mkk_chat_messages: {
        Row: {
          content: string
          created_at: string | null
          id: string
          message_id: string
          message_timestamp: string | null
          metadata: Json | null
          role: string
          session_id: string
        }
        Insert: {
          content: string
          created_at?: string | null
          id?: string
          message_id?: string
          message_timestamp?: string | null
          metadata?: Json | null
          role: string
          session_id: string
        }
        Update: {
          content?: string
          created_at?: string | null
          id?: string
          message_id?: string
          message_timestamp?: string | null
          metadata?: Json | null
          role?: string
          session_id?: string
        }
        Relationships: [
          {
            foreignKeyName: "mkk_chat_messages_session_id_fkey"
            columns: ["session_id"]
            isOneToOne: false
            referencedRelation: "mkk_chat_sessions"
            referencedColumns: ["session_id"]
          },
        ]
      }
      mkk_chat_sessions: {
        Row: {
          collected_data: Json | null
          created_at: string | null
          id: string
          metadata: Json | null
          mode: string
          organization_id: string
          session_id: string
          title: string
          updated_at: string | null
          user_id: string | null
        }
        Insert: {
          collected_data?: Json | null
          created_at?: string | null
          id?: string
          metadata?: Json | null
          mode?: string
          organization_id: string
          session_id?: string
          title?: string
          updated_at?: string | null
          user_id?: string | null
        }
        Update: {
          collected_data?: Json | null
          created_at?: string | null
          id?: string
          metadata?: Json | null
          mode?: string
          organization_id?: string
          session_id?: string
          title?: string
          updated_at?: string | null
          user_id?: string | null
        }
        Relationships: [
          {
            foreignKeyName: "mkk_chat_sessions_organization_id_fkey"
            columns: ["organization_id"]
            isOneToOne: false
            referencedRelation: "organizations"
            referencedColumns: ["id"]
          },
        ]
      }
      notification_logs: {
        Row: {
          call_sid: string
          call_summary_id: string | null
          channel: string
          created_at: string | null
          delivered_at: string | null
          error_message: string | null
          event_data: Json | null
          event_type: string | null
          id: string
          integration_id: string | null
          latency_ms: number | null
          metadata: Json | null
          organization_id: string
          retry_count: number | null
          rule_id: string | null
          sent_at: string | null
          status: string
          success: boolean | null
          updated_at: string | null
        }
        Insert: {
          call_sid: string
          call_summary_id?: string | null
          channel: string
          created_at?: string | null
          delivered_at?: string | null
          error_message?: string | null
          event_data?: Json | null
          event_type?: string | null
          id?: string
          integration_id?: string | null
          latency_ms?: number | null
          metadata?: Json | null
          organization_id: string
          retry_count?: number | null
          rule_id?: string | null
          sent_at?: string | null
          status: string
          success?: boolean | null
          updated_at?: string | null
        }
        Update: {
          call_sid?: string
          call_summary_id?: string | null
          channel?: string
          created_at?: string | null
          delivered_at?: string | null
          error_message?: string | null
          event_data?: Json | null
          event_type?: string | null
          id?: string
          integration_id?: string | null
          latency_ms?: number | null
          metadata?: Json | null
          organization_id?: string
          retry_count?: number | null
          rule_id?: string | null
          sent_at?: string | null
          status?: string
          success?: boolean | null
          updated_at?: string | null
        }
        Relationships: [
          {
            foreignKeyName: "notification_logs_call_summary_id_fkey"
            columns: ["call_summary_id"]
            isOneToOne: false
            referencedRelation: "call_summaries"
            referencedColumns: ["id"]
          },
          {
            foreignKeyName: "notification_logs_organization_id_fkey"
            columns: ["organization_id"]
            isOneToOne: false
            referencedRelation: "organizations"
            referencedColumns: ["id"]
          },
        ]
      }
      notification_rules: {
        Row: {
          conditions: Json | null
          created_at: string | null
          enabled: boolean | null
          event_type: string
          id: string
          integration_id: string
          name: string
          organization_id: string
          priority: number | null
          updated_at: string | null
        }
        Insert: {
          conditions?: Json | null
          created_at?: string | null
          enabled?: boolean | null
          event_type: string
          id?: string
          integration_id: string
          name: string
          organization_id: string
          priority?: number | null
          updated_at?: string | null
        }
        Update: {
          conditions?: Json | null
          created_at?: string | null
          enabled?: boolean | null
          event_type?: string
          id?: string
          integration_id?: string
          name?: string
          organization_id?: string
          priority?: number | null
          updated_at?: string | null
        }
        Relationships: [
          {
            foreignKeyName: "notification_rules_integration_id_fkey"
            columns: ["integration_id"]
            isOneToOne: false
            referencedRelation: "chat_integrations"
            referencedColumns: ["id"]
          },
          {
            foreignKeyName: "notification_rules_organization_id_fkey"
            columns: ["organization_id"]
            isOneToOne: false
            referencedRelation: "organizations"
            referencedColumns: ["id"]
          },
        ]
      }
      number_provisioning_tasks: {
        Row: {
          attempts: number
          created_at: string | null
          error_message: string | null
          id: string
          max_attempts: number
          organization_id: string
          result_number_id: string | null
          status: string
          stripe_event_id: string
          stripe_subscription_id: string
          task_type: string
          updated_at: string | null
        }
        Insert: {
          attempts?: number
          created_at?: string | null
          error_message?: string | null
          id?: string
          max_attempts?: number
          organization_id: string
          result_number_id?: string | null
          status?: string
          stripe_event_id: string
          stripe_subscription_id: string
          task_type: string
          updated_at?: string | null
        }
        Update: {
          attempts?: number
          created_at?: string | null
          error_message?: string | null
          id?: string
          max_attempts?: number
          organization_id?: string
          result_number_id?: string | null
          status?: string
          stripe_event_id?: string
          stripe_subscription_id?: string
          task_type?: string
          updated_at?: string | null
        }
        Relationships: [
          {
            foreignKeyName: "number_provisioning_tasks_organization_id_fkey"
            columns: ["organization_id"]
            isOneToOne: false
            referencedRelation: "organizations"
            referencedColumns: ["id"]
          },
        ]
      }
      ocr_history: {
        Row: {
          completed_at: string | null
          confidence_score: number | null
          created_at: string | null
          document_type: string | null
          engine_used: string | null
          error_message: string | null
          extracted_text: string | null
          file_name: string | null
          file_size: number | null
          file_type: string | null
          id: string
          processing_time_ms: number | null
          request_id: string | null
          status: string | null
          structured_data: Json | null
          subscription_id: string | null
        }
        Insert: {
          completed_at?: string | null
          confidence_score?: number | null
          created_at?: string | null
          document_type?: string | null
          engine_used?: string | null
          error_message?: string | null
          extracted_text?: string | null
          file_name?: string | null
          file_size?: number | null
          file_type?: string | null
          id?: string
          processing_time_ms?: number | null
          request_id?: string | null
          status?: string | null
          structured_data?: Json | null
          subscription_id?: string | null
        }
        Update: {
          completed_at?: string | null
          confidence_score?: number | null
          created_at?: string | null
          document_type?: string | null
          engine_used?: string | null
          error_message?: string | null
          extracted_text?: string | null
          file_name?: string | null
          file_size?: number | null
          file_type?: string | null
          id?: string
          processing_time_ms?: number | null
          request_id?: string | null
          status?: string | null
          structured_data?: Json | null
          subscription_id?: string | null
        }
        Relationships: [
          {
            foreignKeyName: "ocr_history_subscription_id_fkey"
            columns: ["subscription_id"]
            isOneToOne: false
            referencedRelation: "ocr_subscriptions"
            referencedColumns: ["id"]
          },
        ]
      }
      ocr_pricing_plans: {
        Row: {
          created_at: string | null
          display_name: string
          features: Json
          id: string
          max_file_size_mb: number | null
          monthly_pages: number
          name: string
          price_monthly: number
          price_per_extra_page: number | null
          supported_formats: string[] | null
          updated_at: string | null
        }
        Insert: {
          created_at?: string | null
          display_name: string
          features?: Json
          id?: string
          max_file_size_mb?: number | null
          monthly_pages: number
          name: string
          price_monthly: number
          price_per_extra_page?: number | null
          supported_formats?: string[] | null
          updated_at?: string | null
        }
        Update: {
          created_at?: string | null
          display_name?: string
          features?: Json
          id?: string
          max_file_size_mb?: number | null
          monthly_pages?: number
          name?: string
          price_monthly?: number
          price_per_extra_page?: number | null
          supported_formats?: string[] | null
          updated_at?: string | null
        }
        Relationships: []
      }
      ocr_subscriptions: {
        Row: {
          api_key: string
          created_at: string | null
          expires_at: string | null
          features: Json | null
          id: string
          monthly_limit: number
          organization_id: string | null
          plan: string
          status: string | null
          updated_at: string | null
        }
        Insert: {
          api_key?: string
          created_at?: string | null
          expires_at?: string | null
          features?: Json | null
          id?: string
          monthly_limit: number
          organization_id?: string | null
          plan: string
          status?: string | null
          updated_at?: string | null
        }
        Update: {
          api_key?: string
          created_at?: string | null
          expires_at?: string | null
          features?: Json | null
          id?: string
          monthly_limit?: number
          organization_id?: string | null
          plan?: string
          status?: string | null
          updated_at?: string | null
        }
        Relationships: [
          {
            foreignKeyName: "ocr_subscriptions_organization_id_fkey"
            columns: ["organization_id"]
            isOneToOne: true
            referencedRelation: "organizations"
            referencedColumns: ["id"]
          },
        ]
      }
      ocr_usage: {
        Row: {
          count: number | null
          created_at: string | null
          id: string
          metadata: Json | null
          month: string
          subscription_id: string
          updated_at: string | null
        }
        Insert: {
          count?: number | null
          created_at?: string | null
          id?: string
          metadata?: Json | null
          month: string
          subscription_id: string
          updated_at?: string | null
        }
        Update: {
          count?: number | null
          created_at?: string | null
          id?: string
          metadata?: Json | null
          month?: string
          subscription_id?: string
          updated_at?: string | null
        }
        Relationships: [
          {
            foreignKeyName: "ocr_usage_subscription_id_fkey"
            columns: ["subscription_id"]
            isOneToOne: false
            referencedRelation: "ocr_subscriptions"
            referencedColumns: ["id"]
          },
        ]
      }
      organization_members: {
        Row: {
          created_at: string | null
          id: string
          is_active: boolean | null
          organization_id: string | null
          role: string | null
          updated_at: string | null
          user_id: string | null
        }
        Insert: {
          created_at?: string | null
          id?: string
          is_active?: boolean | null
          organization_id?: string | null
          role?: string | null
          updated_at?: string | null
          user_id?: string | null
        }
        Update: {
          created_at?: string | null
          id?: string
          is_active?: boolean | null
          organization_id?: string | null
          role?: string | null
          updated_at?: string | null
          user_id?: string | null
        }
        Relationships: [
          {
            foreignKeyName: "organization_members_organization_id_fkey"
            columns: ["organization_id"]
            isOneToOne: false
            referencedRelation: "organizations"
            referencedColumns: ["id"]
          },
        ]
      }
      organization_settings: {
        Row: {
          ai_configuration_overrides: Json | null
          business_hours: Json | null
          company_name: string
          company_name_kana: string | null
          created_at: string | null
          custom_prompt: string | null
          features_enabled: Json | null
          limitations_override: Json | null
          notification_channels: Json | null
          notification_settings: Json | null
          organization_id: string
          prompt_variables: Json | null
          selected_plan_id: string | null
          updated_at: string | null
        }
        Insert: {
          ai_configuration_overrides?: Json | null
          business_hours?: Json | null
          company_name: string
          company_name_kana?: string | null
          created_at?: string | null
          custom_prompt?: string | null
          features_enabled?: Json | null
          limitations_override?: Json | null
          notification_channels?: Json | null
          notification_settings?: Json | null
          organization_id: string
          prompt_variables?: Json | null
          selected_plan_id?: string | null
          updated_at?: string | null
        }
        Update: {
          ai_configuration_overrides?: Json | null
          business_hours?: Json | null
          company_name?: string
          company_name_kana?: string | null
          created_at?: string | null
          custom_prompt?: string | null
          features_enabled?: Json | null
          limitations_override?: Json | null
          notification_channels?: Json | null
          notification_settings?: Json | null
          organization_id?: string
          prompt_variables?: Json | null
          selected_plan_id?: string | null
          updated_at?: string | null
        }
        Relationships: [
          {
            foreignKeyName: "organization_settings_organization_id_fkey"
            columns: ["organization_id"]
            isOneToOne: true
            referencedRelation: "organizations"
            referencedColumns: ["id"]
          },
          {
            foreignKeyName: "organization_settings_selected_plan_id_fkey"
            columns: ["selected_plan_id"]
            isOneToOne: false
            referencedRelation: "pricing_plans"
            referencedColumns: ["id"]
          },
        ]
      }
      organization_slack_settings: {
        Row: {
          app_id: string
          bot_token: string
          client_id: string
          client_secret: string
          created_at: string | null
          default_channel_prefix: string | null
          id: string
          is_active: boolean | null
          organization_id: string
          scope: string | null
          updated_at: string | null
          webhook_url: string | null
          workspace_id: string
          workspace_name: string | null
        }
        Insert: {
          app_id: string
          bot_token: string
          client_id: string
          client_secret: string
          created_at?: string | null
          default_channel_prefix?: string | null
          id?: string
          is_active?: boolean | null
          organization_id: string
          scope?: string | null
          updated_at?: string | null
          webhook_url?: string | null
          workspace_id: string
          workspace_name?: string | null
        }
        Update: {
          app_id?: string
          bot_token?: string
          client_id?: string
          client_secret?: string
          created_at?: string | null
          default_channel_prefix?: string | null
          id?: string
          is_active?: boolean | null
          organization_id?: string
          scope?: string | null
          updated_at?: string | null
          webhook_url?: string | null
          workspace_id?: string
          workspace_name?: string | null
        }
        Relationships: [
          {
            foreignKeyName: "organization_slack_settings_organization_id_fkey"
            columns: ["organization_id"]
            isOneToOne: true
            referencedRelation: "organizations"
            referencedColumns: ["id"]
          },
        ]
      }
      organizations: {
        Row: {
          created_at: string | null
          domain: string | null
          id: string
          name: string
          slug: string
          stripe_customer_id: string | null
          updated_at: string | null
        }
        Insert: {
          created_at?: string | null
          domain?: string | null
          id?: string
          name: string
          slug: string
          stripe_customer_id?: string | null
          updated_at?: string | null
        }
        Update: {
          created_at?: string | null
          domain?: string | null
          id?: string
          name?: string
          slug?: string
          stripe_customer_id?: string | null
          updated_at?: string | null
        }
        Relationships: []
      }
      outbound_calls: {
        Row: {
          campaign_id: string | null
          created_at: string | null
          created_by: string | null
          duration: number | null
          from_number: string
          id: string
          metadata: Json | null
          organization_id: string
          script_id: string | null
          status: string | null
          to_number: string
          twilio_call_sid: string | null
          updated_at: string | null
        }
        Insert: {
          campaign_id?: string | null
          created_at?: string | null
          created_by?: string | null
          duration?: number | null
          from_number: string
          id?: string
          metadata?: Json | null
          organization_id: string
          script_id?: string | null
          status?: string | null
          to_number: string
          twilio_call_sid?: string | null
          updated_at?: string | null
        }
        Update: {
          campaign_id?: string | null
          created_at?: string | null
          created_by?: string | null
          duration?: number | null
          from_number?: string
          id?: string
          metadata?: Json | null
          organization_id?: string
          script_id?: string | null
          status?: string | null
          to_number?: string
          twilio_call_sid?: string | null
          updated_at?: string | null
        }
        Relationships: [
          {
            foreignKeyName: "outbound_calls_campaign_id_fkey"
            columns: ["campaign_id"]
            isOneToOne: false
            referencedRelation: "autocall_campaigns"
            referencedColumns: ["id"]
          },
          {
            foreignKeyName: "outbound_calls_organization_id_fkey"
            columns: ["organization_id"]
            isOneToOne: false
            referencedRelation: "organizations"
            referencedColumns: ["id"]
          },
          {
            foreignKeyName: "outbound_calls_script_id_fkey"
            columns: ["script_id"]
            isOneToOne: false
            referencedRelation: "scripts"
            referencedColumns: ["id"]
          },
        ]
      }
      outbound_plans: {
        Row: {
          created_at: string | null
          description: string | null
          display_name: string
          features: Json | null
          id: string
          included_minutes: number | null
          is_active: boolean | null
          monthly_price: number
          name: string
          per_minute_price: number
          sort_order: number | null
          stripe_price_id: string | null
          stripe_price_id_yearly: string | null
          updated_at: string | null
          yearly_price: number | null
        }
        Insert: {
          created_at?: string | null
          description?: string | null
          display_name: string
          features?: Json | null
          id?: string
          included_minutes?: number | null
          is_active?: boolean | null
          monthly_price: number
          name: string
          per_minute_price: number
          sort_order?: number | null
          stripe_price_id?: string | null
          stripe_price_id_yearly?: string | null
          updated_at?: string | null
          yearly_price?: number | null
        }
        Update: {
          created_at?: string | null
          description?: string | null
          display_name?: string
          features?: Json | null
          id?: string
          included_minutes?: number | null
          is_active?: boolean | null
          monthly_price?: number
          name?: string
          per_minute_price?: number
          sort_order?: number | null
          stripe_price_id?: string | null
          stripe_price_id_yearly?: string | null
          updated_at?: string | null
          yearly_price?: number | null
        }
        Relationships: []
      }
      phone_call_logs: {
        Row: {
          call_sid: string
          created_at: string | null
          direction: string | null
          duration_seconds: number | null
          ended_at: string | null
          from_number: string | null
          id: string
          metadata: Json | null
          organization_id: string | null
          recording_url: string | null
          started_at: string | null
          status: string | null
          to_number: string | null
          transcription_url: string | null
          updated_at: string | null
        }
        Insert: {
          call_sid: string
          created_at?: string | null
          direction?: string | null
          duration_seconds?: number | null
          ended_at?: string | null
          from_number?: string | null
          id?: string
          metadata?: Json | null
          organization_id?: string | null
          recording_url?: string | null
          started_at?: string | null
          status?: string | null
          to_number?: string | null
          transcription_url?: string | null
          updated_at?: string | null
        }
        Update: {
          call_sid?: string
          created_at?: string | null
          direction?: string | null
          duration_seconds?: number | null
          ended_at?: string | null
          from_number?: string | null
          id?: string
          metadata?: Json | null
          organization_id?: string | null
          recording_url?: string | null
          started_at?: string | null
          status?: string | null
          to_number?: string | null
          transcription_url?: string | null
          updated_at?: string | null
        }
        Relationships: [
          {
            foreignKeyName: "phone_call_logs_organization_id_fkey"
            columns: ["organization_id"]
            isOneToOne: false
            referencedRelation: "organizations"
            referencedColumns: ["id"]
          },
        ]
      }
      phone_number_audit_log: {
        Row: {
          action: string
          error_code: string | null
          error_message: string | null
          executed_at: string
          executed_by: string | null
          id: string
          metadata: Json | null
          monthly_price_jpy: number | null
          organization_id: string
          phone_number: string
          phone_number_id: string | null
          purpose: string | null
          success: boolean
          twilio_account_sid: string | null
          twilio_phone_number_sid: string | null
        }
        Insert: {
          action: string
          error_code?: string | null
          error_message?: string | null
          executed_at?: string
          executed_by?: string | null
          id?: string
          metadata?: Json | null
          monthly_price_jpy?: number | null
          organization_id: string
          phone_number: string
          phone_number_id?: string | null
          purpose?: string | null
          success?: boolean
          twilio_account_sid?: string | null
          twilio_phone_number_sid?: string | null
        }
        Update: {
          action?: string
          error_code?: string | null
          error_message?: string | null
          executed_at?: string
          executed_by?: string | null
          id?: string
          metadata?: Json | null
          monthly_price_jpy?: number | null
          organization_id?: string
          phone_number?: string
          phone_number_id?: string | null
          purpose?: string | null
          success?: boolean
          twilio_account_sid?: string | null
          twilio_phone_number_sid?: string | null
        }
        Relationships: [
          {
            foreignKeyName: "phone_number_audit_log_organization_id_fkey"
            columns: ["organization_id"]
            isOneToOne: false
            referencedRelation: "organizations"
            referencedColumns: ["id"]
          },
          {
            foreignKeyName: "phone_number_audit_log_phone_number_id_fkey"
            columns: ["phone_number_id"]
            isOneToOne: false
            referencedRelation: "phone_numbers"
            referencedColumns: ["id"]
          },
        ]
      }
      phone_numbers: {
        Row: {
          created_at: string | null
          display_name: string | null
          id: string
          is_active: boolean | null
          monthly_price_jpy: number | null
          organization_id: string | null
          phone_number: string
          purchased_at: string | null
          purpose: string | null
          released_at: string | null
          twilio_metadata: Json | null
          twilio_phone_number_sid: string | null
          twilio_status: string | null
          updated_at: string | null
        }
        Insert: {
          created_at?: string | null
          display_name?: string | null
          id?: string
          is_active?: boolean | null
          monthly_price_jpy?: number | null
          organization_id?: string | null
          phone_number: string
          purchased_at?: string | null
          purpose?: string | null
          released_at?: string | null
          twilio_metadata?: Json | null
          twilio_phone_number_sid?: string | null
          twilio_status?: string | null
          updated_at?: string | null
        }
        Update: {
          created_at?: string | null
          display_name?: string | null
          id?: string
          is_active?: boolean | null
          monthly_price_jpy?: number | null
          organization_id?: string | null
          phone_number?: string
          purchased_at?: string | null
          purpose?: string | null
          released_at?: string | null
          twilio_metadata?: Json | null
          twilio_phone_number_sid?: string | null
          twilio_status?: string | null
          updated_at?: string | null
        }
        Relationships: [
          {
            foreignKeyName: "phone_numbers_organization_id_fkey"
            columns: ["organization_id"]
            isOneToOne: false
            referencedRelation: "organizations"
            referencedColumns: ["id"]
          },
        ]
      }
      pricing_plans: {
        Row: {
          created_at: string | null
          display_name: string
          features: Json | null
          id: string
          included_calls: number | null
          is_active: boolean | null
          limitations: Json | null
          monthly_price: number
          name: string
          per_call_price: number
          per_call_price_yearly: number
          sort_order: number | null
          stripe_price_id: string | null
          stripe_price_id_yearly: string | null
          updated_at: string | null
          yearly_price: number
        }
        Insert: {
          created_at?: string | null
          display_name: string
          features?: Json | null
          id?: string
          included_calls?: number | null
          is_active?: boolean | null
          limitations?: Json | null
          monthly_price: number
          name: string
          per_call_price: number
          per_call_price_yearly: number
          sort_order?: number | null
          stripe_price_id?: string | null
          stripe_price_id_yearly?: string | null
          updated_at?: string | null
          yearly_price: number
        }
        Update: {
          created_at?: string | null
          display_name?: string
          features?: Json | null
          id?: string
          included_calls?: number | null
          is_active?: boolean | null
          limitations?: Json | null
          monthly_price?: number
          name?: string
          per_call_price?: number
          per_call_price_yearly?: number
          sort_order?: number | null
          stripe_price_id?: string | null
          stripe_price_id_yearly?: string | null
          updated_at?: string | null
          yearly_price?: number
        }
        Relationships: []
      }
      profiles: {
        Row: {
          avatar_url: string | null
          created_at: string
          email: string | null
          full_name: string | null
          google_access_token: string | null
          google_refresh_token: string | null
          id: string
          updated_at: string
        }
        Insert: {
          avatar_url?: string | null
          created_at?: string
          email?: string | null
          full_name?: string | null
          google_access_token?: string | null
          google_refresh_token?: string | null
          id: string
          updated_at?: string
        }
        Update: {
          avatar_url?: string | null
          created_at?: string
          email?: string | null
          full_name?: string | null
          google_access_token?: string | null
          google_refresh_token?: string | null
          id?: string
          updated_at?: string
        }
        Relationships: []
      }
      project_interests: {
        Row: {
          created_at: string | null
          feedback_reason: string | null
          id: string
          notes: string | null
          project_id: string | null
          slack_user_id: string
          slack_user_name: string | null
          status: string | null
          updated_at: string | null
        }
        Insert: {
          created_at?: string | null
          feedback_reason?: string | null
          id?: string
          notes?: string | null
          project_id?: string | null
          slack_user_id: string
          slack_user_name?: string | null
          status?: string | null
          updated_at?: string | null
        }
        Update: {
          created_at?: string | null
          feedback_reason?: string | null
          id?: string
          notes?: string | null
          project_id?: string | null
          slack_user_id?: string
          slack_user_name?: string | null
          status?: string | null
          updated_at?: string | null
        }
        Relationships: [
          {
            foreignKeyName: "project_interests_project_id_fkey"
            columns: ["project_id"]
            isOneToOne: false
            referencedRelation: "projects"
            referencedColumns: ["id"]
          },
        ]
      }
      projects: {
        Row: {
          company_name: string | null
          contract_type: string | null
          created_at: string | null
          description: string | null
          duration: string | null
          hours_per_month: string | null
          id: string
          location: string | null
          organization_id: string | null
          overview: string | null
          published_at: string | null
          rate: string | null
          required_skills: string[] | null
          start_date: string | null
          status: string | null
          title: string
          updated_at: string | null
          welcome_skills: string[] | null
          work_style: string | null
        }
        Insert: {
          company_name?: string | null
          contract_type?: string | null
          created_at?: string | null
          description?: string | null
          duration?: string | null
          hours_per_month?: string | null
          id?: string
          location?: string | null
          organization_id?: string | null
          overview?: string | null
          published_at?: string | null
          rate?: string | null
          required_skills?: string[] | null
          start_date?: string | null
          status?: string | null
          title: string
          updated_at?: string | null
          welcome_skills?: string[] | null
          work_style?: string | null
        }
        Update: {
          company_name?: string | null
          contract_type?: string | null
          created_at?: string | null
          description?: string | null
          duration?: string | null
          hours_per_month?: string | null
          id?: string
          location?: string | null
          organization_id?: string | null
          overview?: string | null
          published_at?: string | null
          rate?: string | null
          required_skills?: string[] | null
          start_date?: string | null
          status?: string | null
          title?: string
          updated_at?: string | null
          welcome_skills?: string[] | null
          work_style?: string | null
        }
        Relationships: [
          {
            foreignKeyName: "projects_organization_id_fkey"
            columns: ["organization_id"]
            isOneToOne: false
            referencedRelation: "organizations"
            referencedColumns: ["id"]
          },
        ]
      }
      prompt_templates: {
        Row: {
          base_prompt: string
          created_at: string | null
          id: string
          input_audio_transcription: boolean | null
          is_active: boolean | null
          is_default: boolean | null
          max_response_tokens: number | null
          model: string | null
          name: string
          plan_id: string | null
          system_prompt: string
          temperature: number | null
          tools_config: Json | null
          turn_detection: Json | null
          updated_at: string | null
          variables: Json | null
          voice: string | null
        }
        Insert: {
          base_prompt: string
          created_at?: string | null
          id?: string
          input_audio_transcription?: boolean | null
          is_active?: boolean | null
          is_default?: boolean | null
          max_response_tokens?: number | null
          model?: string | null
          name: string
          plan_id?: string | null
          system_prompt: string
          temperature?: number | null
          tools_config?: Json | null
          turn_detection?: Json | null
          updated_at?: string | null
          variables?: Json | null
          voice?: string | null
        }
        Update: {
          base_prompt?: string
          created_at?: string | null
          id?: string
          input_audio_transcription?: boolean | null
          is_active?: boolean | null
          is_default?: boolean | null
          max_response_tokens?: number | null
          model?: string | null
          name?: string
          plan_id?: string | null
          system_prompt?: string
          temperature?: number | null
          tools_config?: Json | null
          turn_detection?: Json | null
          updated_at?: string | null
          variables?: Json | null
          voice?: string | null
        }
        Relationships: [
          {
            foreignKeyName: "prompt_templates_plan_id_fkey"
            columns: ["plan_id"]
            isOneToOne: false
            referencedRelation: "pricing_plans"
            referencedColumns: ["id"]
          },
        ]
      }
      prompt_variables: {
        Row: {
          created_at: string | null
          default_value: string | null
          id: string
          is_required: boolean | null
          sort_order: number | null
          updated_at: string | null
          validation_rules: Json | null
          variable_key: string
          variable_label: string
          variable_type: string
        }
        Insert: {
          created_at?: string | null
          default_value?: string | null
          id?: string
          is_required?: boolean | null
          sort_order?: number | null
          updated_at?: string | null
          validation_rules?: Json | null
          variable_key: string
          variable_label: string
          variable_type: string
        }
        Update: {
          created_at?: string | null
          default_value?: string | null
          id?: string
          is_required?: boolean | null
          sort_order?: number | null
          updated_at?: string | null
          validation_rules?: Json | null
          variable_key?: string
          variable_label?: string
          variable_type?: string
        }
        Relationships: []
      }
      scripts: {
        Row: {
          content: string
          created_at: string | null
          created_by: string | null
          id: string
          language: string | null
          metadata: Json | null
          name: string
          organization_id: string
          updated_at: string | null
        }
        Insert: {
          content: string
          created_at?: string | null
          created_by?: string | null
          id?: string
          language?: string | null
          metadata?: Json | null
          name: string
          organization_id: string
          updated_at?: string | null
        }
        Update: {
          content?: string
          created_at?: string | null
          created_by?: string | null
          id?: string
          language?: string | null
          metadata?: Json | null
          name?: string
          organization_id?: string
          updated_at?: string | null
        }
        Relationships: [
          {
            foreignKeyName: "scripts_organization_id_fkey"
            columns: ["organization_id"]
            isOneToOne: false
            referencedRelation: "organizations"
            referencedColumns: ["id"]
          },
        ]
      }
      service_subscriptions: {
        Row: {
          cancelled_at: string | null
          created_at: string | null
          current_period_end: string | null
          current_period_start: string | null
          id: string
          organization_id: string
          plan_id: string
          service_type: string
          status: string | null
          stripe_price_id: string | null
          stripe_subscription_id: string | null
          trial_end: string | null
          updated_at: string | null
          usage_this_period: number | null
        }
        Insert: {
          cancelled_at?: string | null
          created_at?: string | null
          current_period_end?: string | null
          current_period_start?: string | null
          id?: string
          organization_id: string
          plan_id: string
          service_type: string
          status?: string | null
          stripe_price_id?: string | null
          stripe_subscription_id?: string | null
          trial_end?: string | null
          updated_at?: string | null
          usage_this_period?: number | null
        }
        Update: {
          cancelled_at?: string | null
          created_at?: string | null
          current_period_end?: string | null
          current_period_start?: string | null
          id?: string
          organization_id?: string
          plan_id?: string
          service_type?: string
          status?: string | null
          stripe_price_id?: string | null
          stripe_subscription_id?: string | null
          trial_end?: string | null
          updated_at?: string | null
          usage_this_period?: number | null
        }
        Relationships: [
          {
            foreignKeyName: "service_subscriptions_organization_id_fkey"
            columns: ["organization_id"]
            isOneToOne: false
            referencedRelation: "organizations"
            referencedColumns: ["id"]
          },
          {
            foreignKeyName: "service_subscriptions_plan_id_fkey"
            columns: ["plan_id"]
            isOneToOne: false
            referencedRelation: "pricing_plans"
            referencedColumns: ["id"]
          },
        ]
      }
      service_usage: {
        Row: {
          billable: boolean | null
          billing_period_end: string | null
          billing_period_start: string | null
          created_at: string | null
          duration_seconds: number | null
          id: string
          organization_id: string
          pages: number | null
          related_id: string | null
          service_type: string
          total_price: number | null
          unit_price: number | null
          usage_count: number | null
          usage_type: string
        }
        Insert: {
          billable?: boolean | null
          billing_period_end?: string | null
          billing_period_start?: string | null
          created_at?: string | null
          duration_seconds?: number | null
          id?: string
          organization_id: string
          pages?: number | null
          related_id?: string | null
          service_type: string
          total_price?: number | null
          unit_price?: number | null
          usage_count?: number | null
          usage_type: string
        }
        Update: {
          billable?: boolean | null
          billing_period_end?: string | null
          billing_period_start?: string | null
          created_at?: string | null
          duration_seconds?: number | null
          id?: string
          organization_id?: string
          pages?: number | null
          related_id?: string | null
          service_type?: string
          total_price?: number | null
          unit_price?: number | null
          usage_count?: number | null
          usage_type?: string
        }
        Relationships: [
          {
            foreignKeyName: "service_usage_organization_id_fkey"
            columns: ["organization_id"]
            isOneToOne: false
            referencedRelation: "organizations"
            referencedColumns: ["id"]
          },
        ]
      }
      slack_connect_channels: {
        Row: {
          channel_id: string
          channel_name: string
          connected_at: string | null
          created_at: string | null
          customer_email: string
          customer_workspace_id: string | null
          customer_workspace_name: string | null
          disconnected_at: string | null
          id: string
          invite_id: string | null
          metadata: Json | null
          organization_id: string
          status: string
        }
        Insert: {
          channel_id: string
          channel_name: string
          connected_at?: string | null
          created_at?: string | null
          customer_email: string
          customer_workspace_id?: string | null
          customer_workspace_name?: string | null
          disconnected_at?: string | null
          id?: string
          invite_id?: string | null
          metadata?: Json | null
          organization_id: string
          status?: string
        }
        Update: {
          channel_id?: string
          channel_name?: string
          connected_at?: string | null
          created_at?: string | null
          customer_email?: string
          customer_workspace_id?: string | null
          customer_workspace_name?: string | null
          disconnected_at?: string | null
          id?: string
          invite_id?: string | null
          metadata?: Json | null
          organization_id?: string
          status?: string
        }
        Relationships: [
          {
            foreignKeyName: "slack_connect_channels_invite_id_fkey"
            columns: ["invite_id"]
            isOneToOne: false
            referencedRelation: "slack_connect_invites"
            referencedColumns: ["id"]
          },
          {
            foreignKeyName: "slack_connect_channels_organization_id_fkey"
            columns: ["organization_id"]
            isOneToOne: false
            referencedRelation: "organizations"
            referencedColumns: ["id"]
          },
        ]
      }
      slack_connect_invites: {
        Row: {
          created_at: string | null
          customer_email: string | null
          expires_at: string
          id: string
          metadata: Json | null
          organization_id: string
          status: string
          token: string
          used_at: string | null
        }
        Insert: {
          created_at?: string | null
          customer_email?: string | null
          expires_at: string
          id?: string
          metadata?: Json | null
          organization_id: string
          status?: string
          token?: string
          used_at?: string | null
        }
        Update: {
          created_at?: string | null
          customer_email?: string | null
          expires_at?: string
          id?: string
          metadata?: Json | null
          organization_id?: string
          status?: string
          token?: string
          used_at?: string | null
        }
        Relationships: [
          {
            foreignKeyName: "slack_connect_invites_organization_id_fkey"
            columns: ["organization_id"]
            isOneToOne: false
            referencedRelation: "organizations"
            referencedColumns: ["id"]
          },
        ]
      }
      slack_user_profiles: {
        Row: {
          bio: string | null
          created_at: string | null
          desired_rate: string | null
          email: string | null
          experience_years: string | null
          id: string
          name: string | null
          skills: string[] | null
          slack_team_id: string | null
          slack_user_id: string
          updated_at: string | null
          work_styles: string[] | null
        }
        Insert: {
          bio?: string | null
          created_at?: string | null
          desired_rate?: string | null
          email?: string | null
          experience_years?: string | null
          id?: string
          name?: string | null
          skills?: string[] | null
          slack_team_id?: string | null
          slack_user_id: string
          updated_at?: string | null
          work_styles?: string[] | null
        }
        Update: {
          bio?: string | null
          created_at?: string | null
          desired_rate?: string | null
          email?: string | null
          experience_years?: string | null
          id?: string
          name?: string | null
          skills?: string[] | null
          slack_team_id?: string | null
          slack_user_id?: string
          updated_at?: string | null
          work_styles?: string[] | null
        }
        Relationships: []
      }
      subscriptions: {
        Row: {
          created_at: string | null
          current_period_end: string | null
          current_period_start: string | null
          id: string
          organization_id: string | null
          plan_id: string | null
          status: string | null
          stripe_customer_id: string | null
          stripe_subscription_id: string | null
          updated_at: string | null
        }
        Insert: {
          created_at?: string | null
          current_period_end?: string | null
          current_period_start?: string | null
          id?: string
          organization_id?: string | null
          plan_id?: string | null
          status?: string | null
          stripe_customer_id?: string | null
          stripe_subscription_id?: string | null
          updated_at?: string | null
        }
        Update: {
          created_at?: string | null
          current_period_end?: string | null
          current_period_start?: string | null
          id?: string
          organization_id?: string | null
          plan_id?: string | null
          status?: string | null
          stripe_customer_id?: string | null
          stripe_subscription_id?: string | null
          updated_at?: string | null
        }
        Relationships: [
          {
            foreignKeyName: "subscriptions_organization_id_fkey"
            columns: ["organization_id"]
            isOneToOne: false
            referencedRelation: "organizations"
            referencedColumns: ["id"]
          },
          {
            foreignKeyName: "subscriptions_plan_id_fkey"
            columns: ["plan_id"]
            isOneToOne: false
            referencedRelation: "pricing_plans"
            referencedColumns: ["id"]
          },
        ]
      }
      system_parameter_master: {
        Row: {
          category: string | null
          created_at: string | null
          description: string | null
          id: number
          is_active: boolean | null
          organization_id: string
          parameter_key: string
          parameter_type: string | null
          parameter_value: string | null
          updated_at: string | null
        }
        Insert: {
          category?: string | null
          created_at?: string | null
          description?: string | null
          id?: number
          is_active?: boolean | null
          organization_id: string
          parameter_key: string
          parameter_type?: string | null
          parameter_value?: string | null
          updated_at?: string | null
        }
        Update: {
          category?: string | null
          created_at?: string | null
          description?: string | null
          id?: number
          is_active?: boolean | null
          organization_id?: string
          parameter_key?: string
          parameter_type?: string | null
          parameter_value?: string | null
          updated_at?: string | null
        }
        Relationships: [
          {
            foreignKeyName: "system_parameter_master_organization_id_fkey"
            columns: ["organization_id"]
            isOneToOne: false
            referencedRelation: "organizations"
            referencedColumns: ["id"]
          },
        ]
      }
      system_settings: {
        Row: {
          category: string | null
          created_at: string | null
          description: string | null
          id: string
          is_sensitive: boolean | null
          key: string
          updated_at: string | null
          value: string
          value_type: string | null
        }
        Insert: {
          category?: string | null
          created_at?: string | null
          description?: string | null
          id?: string
          is_sensitive?: boolean | null
          key: string
          updated_at?: string | null
          value: string
          value_type?: string | null
        }
        Update: {
          category?: string | null
          created_at?: string | null
          description?: string | null
          id?: string
          is_sensitive?: boolean | null
          key?: string
          updated_at?: string | null
          value?: string
          value_type?: string | null
        }
        Relationships: []
      }
      tank_design_calculations: {
        Row: {
          created_at: string | null
          created_by: string | null
          dimensions: Json
          foundation: Json
          id: string
          input_params: Json
          loadings: Json
          organization_id: string
          report_url: string | null
          status: string | null
          updated_at: string | null
        }
        Insert: {
          created_at?: string | null
          created_by?: string | null
          dimensions: Json
          foundation: Json
          id?: string
          input_params: Json
          loadings: Json
          organization_id: string
          report_url?: string | null
          status?: string | null
          updated_at?: string | null
        }
        Update: {
          created_at?: string | null
          created_by?: string | null
          dimensions?: Json
          foundation?: Json
          id?: string
          input_params?: Json
          loadings?: Json
          organization_id?: string
          report_url?: string | null
          status?: string | null
          updated_at?: string | null
        }
        Relationships: [
          {
            foreignKeyName: "tank_design_calculations_organization_id_fkey"
            columns: ["organization_id"]
            isOneToOne: false
            referencedRelation: "organizations"
            referencedColumns: ["id"]
          },
        ]
      }
      tasks: {
        Row: {
          category_id: string | null
          completed_at: string | null
          created_at: string
          description: string | null
          due_date: string | null
          due_time: string | null
          estimated_minutes: number | null
          google_calendar_event_id: string | null
          google_task_id: string | null
          id: string
          is_backlog: boolean
          parent_id: string | null
          priority: number
          sort_order: number
          status: string
          title: string
          updated_at: string
          user_id: string
        }
        Insert: {
          category_id?: string | null
          completed_at?: string | null
          created_at?: string
          description?: string | null
          due_date?: string | null
          due_time?: string | null
          estimated_minutes?: number | null
          google_calendar_event_id?: string | null
          google_task_id?: string | null
          id?: string
          is_backlog?: boolean
          parent_id?: string | null
          priority?: number
          sort_order?: number
          status?: string
          title: string
          updated_at?: string
          user_id: string
        }
        Update: {
          category_id?: string | null
          completed_at?: string | null
          created_at?: string
          description?: string | null
          due_date?: string | null
          due_time?: string | null
          estimated_minutes?: number | null
          google_calendar_event_id?: string | null
          google_task_id?: string | null
          id?: string
          is_backlog?: boolean
          parent_id?: string | null
          priority?: number
          sort_order?: number
          status?: string
          title?: string
          updated_at?: string
          user_id?: string
        }
        Relationships: [
          {
            foreignKeyName: "tasks_category_id_fkey"
            columns: ["category_id"]
            isOneToOne: false
            referencedRelation: "categories"
            referencedColumns: ["id"]
          },
          {
            foreignKeyName: "tasks_parent_id_fkey"
            columns: ["parent_id"]
            isOneToOne: false
            referencedRelation: "tasks"
            referencedColumns: ["id"]
          },
        ]
      }
      twilio_webhooks: {
        Row: {
          created_at: string | null
          id: string
          params: Json
          type: string
        }
        Insert: {
          created_at?: string | null
          id?: string
          params: Json
          type: string
        }
        Update: {
          created_at?: string | null
          id?: string
          params?: Json
          type?: string
        }
        Relationships: []
      }
      unified_phone_numbers: {
        Row: {
          created_at: string | null
          display_name: string | null
          id: string
          inbound_config: Json | null
          inbound_enabled: boolean | null
          inbound_plan_id: string | null
          is_plan_included: boolean
          organization_id: string
          outbound_config: Json | null
          outbound_enabled: boolean | null
          outbound_plan_id: string | null
          phone_number: string
          prompt_template_id: string | null
          status: string | null
          stripe_subscription_id: string | null
          stripe_subscription_item_id: string | null
          twilio_sid: string | null
          updated_at: string | null
        }
        Insert: {
          created_at?: string | null
          display_name?: string | null
          id?: string
          inbound_config?: Json | null
          inbound_enabled?: boolean | null
          inbound_plan_id?: string | null
          is_plan_included?: boolean
          organization_id: string
          outbound_config?: Json | null
          outbound_enabled?: boolean | null
          outbound_plan_id?: string | null
          phone_number: string
          prompt_template_id?: string | null
          status?: string | null
          stripe_subscription_id?: string | null
          stripe_subscription_item_id?: string | null
          twilio_sid?: string | null
          updated_at?: string | null
        }
        Update: {
          created_at?: string | null
          display_name?: string | null
          id?: string
          inbound_config?: Json | null
          inbound_enabled?: boolean | null
          inbound_plan_id?: string | null
          is_plan_included?: boolean
          organization_id?: string
          outbound_config?: Json | null
          outbound_enabled?: boolean | null
          outbound_plan_id?: string | null
          phone_number?: string
          prompt_template_id?: string | null
          status?: string | null
          stripe_subscription_id?: string | null
          stripe_subscription_item_id?: string | null
          twilio_sid?: string | null
          updated_at?: string | null
        }
        Relationships: [
          {
            foreignKeyName: "unified_phone_numbers_inbound_plan_id_fkey"
            columns: ["inbound_plan_id"]
            isOneToOne: false
            referencedRelation: "pricing_plans"
            referencedColumns: ["id"]
          },
          {
            foreignKeyName: "unified_phone_numbers_organization_id_fkey"
            columns: ["organization_id"]
            isOneToOne: false
            referencedRelation: "organizations"
            referencedColumns: ["id"]
          },
          {
            foreignKeyName: "unified_phone_numbers_outbound_plan_id_fkey"
            columns: ["outbound_plan_id"]
            isOneToOne: false
            referencedRelation: "outbound_plans"
            referencedColumns: ["id"]
          },
          {
            foreignKeyName: "unified_phone_numbers_prompt_template_id_fkey"
            columns: ["prompt_template_id"]
            isOneToOne: false
            referencedRelation: "prompt_templates"
            referencedColumns: ["id"]
          },
        ]
      }
      users: {
        Row: {
          created_at: string | null
          email: string
          full_name: string | null
          id: string
          is_active: boolean | null
          organization_id: string | null
          role: string | null
          updated_at: string | null
        }
        Insert: {
          created_at?: string | null
          email: string
          full_name?: string | null
          id: string
          is_active?: boolean | null
          organization_id?: string | null
          role?: string | null
          updated_at?: string | null
        }
        Update: {
          created_at?: string | null
          email?: string
          full_name?: string | null
          id?: string
          is_active?: boolean | null
          organization_id?: string | null
          role?: string | null
          updated_at?: string | null
        }
        Relationships: [
          {
            foreignKeyName: "users_organization_id_fkey"
            columns: ["organization_id"]
            isOneToOne: false
            referencedRelation: "organizations"
            referencedColumns: ["id"]
          },
        ]
      }
    }
    Views: {
      inbound_phone_numbers: {
        Row: {
          created_at: string | null
          display_name: string | null
          id: string | null
          inbound_config: Json | null
          inbound_plan_id: string | null
          organization_id: string | null
          phone_number: string | null
          plan_display_name: string | null
          plan_name: string | null
          prompt_name: string | null
          prompt_template_id: string | null
          status: string | null
        }
        Relationships: [
          {
            foreignKeyName: "unified_phone_numbers_inbound_plan_id_fkey"
            columns: ["inbound_plan_id"]
            isOneToOne: false
            referencedRelation: "pricing_plans"
            referencedColumns: ["id"]
          },
          {
            foreignKeyName: "unified_phone_numbers_organization_id_fkey"
            columns: ["organization_id"]
            isOneToOne: false
            referencedRelation: "organizations"
            referencedColumns: ["id"]
          },
          {
            foreignKeyName: "unified_phone_numbers_prompt_template_id_fkey"
            columns: ["prompt_template_id"]
            isOneToOne: false
            referencedRelation: "prompt_templates"
            referencedColumns: ["id"]
          },
        ]
      }
      outbound_phone_numbers: {
        Row: {
          created_at: string | null
          display_name: string | null
          id: string | null
          organization_id: string | null
          outbound_config: Json | null
          outbound_plan_id: string | null
          phone_number: string | null
          plan_display_name: string | null
          plan_name: string | null
          status: string | null
        }
        Relationships: [
          {
            foreignKeyName: "unified_phone_numbers_organization_id_fkey"
            columns: ["organization_id"]
            isOneToOne: false
            referencedRelation: "organizations"
            referencedColumns: ["id"]
          },
          {
            foreignKeyName: "unified_phone_numbers_outbound_plan_id_fkey"
            columns: ["outbound_plan_id"]
            isOneToOne: false
            referencedRelation: "outbound_plans"
            referencedColumns: ["id"]
          },
        ]
      }
    }
    Functions: {
      calculate_monthly_bill: {
        Args: { org_id: string }
        Returns: {
          base_price: number
          plan_name: string
          service_type: string
          total_price: number
          usage_charges: number
        }[]
      }
      check_admin_access: {
        Args: { permission?: string; user_email: string }
        Returns: boolean
      }
      cleanup_old_notification_logs: { Args: never; Returns: undefined }
      create_chat_session_with_greeting: {
        Args: { p_mode?: string; p_title?: string }
        Returns: Json
      }
      get_chat_history: { Args: { p_limit?: number }; Returns: Json }
      has_active_subscription: {
        Args: { org_id: string; service: string }
        Returns: boolean
      }
      has_fax_pro_plan: { Args: { org_id: string }; Returns: boolean }
      is_organization_member: { Args: { org_id: string }; Returns: boolean }
      save_chat_message: {
        Args: {
          p_content: string
          p_metadata?: Json
          p_role: string
          p_session_id: string
        }
        Returns: Json
      }
      update_expired_invites: { Args: never; Returns: undefined }
      verify_infisical_access: { Args: never; Returns: boolean }
    }
    Enums: {
      [_ in never]: never
    }
    CompositeTypes: {
      [_ in never]: never
    }
  }
}

type DatabaseWithoutInternals = Omit<Database, "__InternalSupabase">

type DefaultSchema = DatabaseWithoutInternals[Extract<keyof Database, "public">]

export type Tables<
  DefaultSchemaTableNameOrOptions extends
    | keyof (DefaultSchema["Tables"] & DefaultSchema["Views"])
    | { schema: keyof DatabaseWithoutInternals },
  TableName extends DefaultSchemaTableNameOrOptions extends {
    schema: keyof DatabaseWithoutInternals
  }
    ? keyof (DatabaseWithoutInternals[DefaultSchemaTableNameOrOptions["schema"]]["Tables"] &
        DatabaseWithoutInternals[DefaultSchemaTableNameOrOptions["schema"]]["Views"])
    : never = never,
> = DefaultSchemaTableNameOrOptions extends {
  schema: keyof DatabaseWithoutInternals
}
  ? (DatabaseWithoutInternals[DefaultSchemaTableNameOrOptions["schema"]]["Tables"] &
      DatabaseWithoutInternals[DefaultSchemaTableNameOrOptions["schema"]]["Views"])[TableName] extends {
      Row: infer R
    }
    ? R
    : never
  : DefaultSchemaTableNameOrOptions extends keyof (DefaultSchema["Tables"] &
        DefaultSchema["Views"])
    ? (DefaultSchema["Tables"] &
        DefaultSchema["Views"])[DefaultSchemaTableNameOrOptions] extends {
        Row: infer R
      }
      ? R
      : never
    : never

export type TablesInsert<
  DefaultSchemaTableNameOrOptions extends
    | keyof DefaultSchema["Tables"]
    | { schema: keyof DatabaseWithoutInternals },
  TableName extends DefaultSchemaTableNameOrOptions extends {
    schema: keyof DatabaseWithoutInternals
  }
    ? keyof DatabaseWithoutInternals[DefaultSchemaTableNameOrOptions["schema"]]["Tables"]
    : never = never,
> = DefaultSchemaTableNameOrOptions extends {
  schema: keyof DatabaseWithoutInternals
}
  ? DatabaseWithoutInternals[DefaultSchemaTableNameOrOptions["schema"]]["Tables"][TableName] extends {
      Insert: infer I
    }
    ? I
    : never
  : DefaultSchemaTableNameOrOptions extends keyof DefaultSchema["Tables"]
    ? DefaultSchema["Tables"][DefaultSchemaTableNameOrOptions] extends {
        Insert: infer I
      }
      ? I
      : never
    : never

export type TablesUpdate<
  DefaultSchemaTableNameOrOptions extends
    | keyof DefaultSchema["Tables"]
    | { schema: keyof DatabaseWithoutInternals },
  TableName extends DefaultSchemaTableNameOrOptions extends {
    schema: keyof DatabaseWithoutInternals
  }
    ? keyof DatabaseWithoutInternals[DefaultSchemaTableNameOrOptions["schema"]]["Tables"]
    : never = never,
> = DefaultSchemaTableNameOrOptions extends {
  schema: keyof DatabaseWithoutInternals
}
  ? DatabaseWithoutInternals[DefaultSchemaTableNameOrOptions["schema"]]["Tables"][TableName] extends {
      Update: infer U
    }
    ? U
    : never
  : DefaultSchemaTableNameOrOptions extends keyof DefaultSchema["Tables"]
    ? DefaultSchema["Tables"][DefaultSchemaTableNameOrOptions] extends {
        Update: infer U
      }
      ? U
      : never
    : never

export type Enums<
  DefaultSchemaEnumNameOrOptions extends
    | keyof DefaultSchema["Enums"]
    | { schema: keyof DatabaseWithoutInternals },
  EnumName extends DefaultSchemaEnumNameOrOptions extends {
    schema: keyof DatabaseWithoutInternals
  }
    ? keyof DatabaseWithoutInternals[DefaultSchemaEnumNameOrOptions["schema"]]["Enums"]
    : never = never,
> = DefaultSchemaEnumNameOrOptions extends {
  schema: keyof DatabaseWithoutInternals
}
  ? DatabaseWithoutInternals[DefaultSchemaEnumNameOrOptions["schema"]]["Enums"][EnumName]
  : DefaultSchemaEnumNameOrOptions extends keyof DefaultSchema["Enums"]
    ? DefaultSchema["Enums"][DefaultSchemaEnumNameOrOptions]
    : never

export type CompositeTypes<
  PublicCompositeTypeNameOrOptions extends
    | keyof DefaultSchema["CompositeTypes"]
    | { schema: keyof DatabaseWithoutInternals },
  CompositeTypeName extends PublicCompositeTypeNameOrOptions extends {
    schema: keyof DatabaseWithoutInternals
  }
    ? keyof DatabaseWithoutInternals[PublicCompositeTypeNameOrOptions["schema"]]["CompositeTypes"]
    : never = never,
> = PublicCompositeTypeNameOrOptions extends {
  schema: keyof DatabaseWithoutInternals
}
  ? DatabaseWithoutInternals[PublicCompositeTypeNameOrOptions["schema"]]["CompositeTypes"][CompositeTypeName]
  : PublicCompositeTypeNameOrOptions extends keyof DefaultSchema["CompositeTypes"]
    ? DefaultSchema["CompositeTypes"][PublicCompositeTypeNameOrOptions]
    : never

export const Constants = {
  public: {
    Enums: {},
  },
} as const
