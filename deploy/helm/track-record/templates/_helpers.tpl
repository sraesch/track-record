{{/* Names ------------------------------------------------------------------ */}}

{{- define "track-record.name" -}}
{{- default .Chart.Name .Values.nameOverride | trunc 63 | trimSuffix "-" }}
{{- end }}

{{/* Release name, or <release>-track-record if it does not contain the chart name. */}}
{{- define "track-record.fullname" -}}
{{- if .Values.fullnameOverride }}
{{- .Values.fullnameOverride | trunc 63 | trimSuffix "-" }}
{{- else }}
{{- $name := default .Chart.Name .Values.nameOverride }}
{{- if contains $name .Release.Name }}
{{- .Release.Name | trunc 63 | trimSuffix "-" }}
{{- else }}
{{- printf "%s-%s" .Release.Name $name | trunc 63 | trimSuffix "-" }}
{{- end }}
{{- end }}
{{- end }}

{{/* <fullname>-<suffix>, for the components. Call with (list $ "suffix"). */}}
{{- define "track-record.component" -}}
{{- $root := index . 0 }}
{{- printf "%s-%s" (include "track-record.fullname" $root | trunc 40 | trimSuffix "-") (index . 1) }}
{{- end }}

{{/* Labels ----------------------------------------------------------------- */}}

{{- define "track-record.selectorLabels" -}}
app.kubernetes.io/name: {{ include "track-record.name" . }}
app.kubernetes.io/instance: {{ .Release.Name }}
{{- end }}

{{- define "track-record.labels" -}}
helm.sh/chart: {{ printf "%s-%s" .Chart.Name .Chart.Version | replace "+" "_" | trunc 63 | trimSuffix "-" }}
{{ include "track-record.selectorLabels" . }}
app.kubernetes.io/version: {{ include "track-record.imageTag" . | quote }}
app.kubernetes.io/managed-by: {{ .Release.Service }}
app.kubernetes.io/part-of: track-record
{{- with .Values.commonLabels }}
{{ toYaml . }}
{{- end }}
{{- end }}

{{/* Labels of one component. Call with (list $ "server"). */}}
{{- define "track-record.componentLabels" -}}
{{ include "track-record.labels" (index . 0) }}
app.kubernetes.io/component: {{ index . 1 }}
{{- end }}

{{- define "track-record.componentSelectorLabels" -}}
{{ include "track-record.selectorLabels" (index . 0) }}
app.kubernetes.io/component: {{ index . 1 }}
{{- end }}

{{/* Images ----------------------------------------------------------------- */}}

{{- define "track-record.imageTag" -}}
{{- default .Chart.AppVersion .Values.imageTag }}
{{- end }}

{{/* repository:tag of one component image. Call with (list $ .Values.server.image). */}}
{{- define "track-record.image" -}}
{{- $root := index . 0 }}
{{- $image := index . 1 }}
{{- printf "%s:%s" $image.repository (default (include "track-record.imageTag" $root) $image.tag) }}
{{- end }}

{{- define "track-record.imagePullSecrets" -}}
{{- with .Values.imagePullSecrets -}}
imagePullSecrets:
  {{- toYaml . | nindent 2 }}
{{- end }}
{{- end }}

{{/* Database connection ------------------------------------------------------ */}}

{{- define "track-record.databaseHost" -}}
{{- if .Values.database.enabled }}
{{- include "track-record.component" (list . "database") }}
{{- else }}
{{- required "externalDatabase.host is required when database.enabled is false" .Values.externalDatabase.host }}
{{- end }}
{{- end }}

{{- define "track-record.databasePort" -}}
{{- if .Values.database.enabled }}5432{{ else }}{{ .Values.externalDatabase.port }}{{ end }}
{{- end }}

{{- define "track-record.databaseName" -}}
{{- if .Values.database.enabled }}track_record{{ else }}{{ .Values.externalDatabase.database }}{{ end }}
{{- end }}

{{- define "track-record.databaseUser" -}}
{{- if .Values.database.enabled }}postgres{{ else }}{{ .Values.externalDatabase.user }}{{ end }}
{{- end }}

{{- define "track-record.databaseSslMode" -}}
{{- if .Values.database.enabled }}disable{{ else }}{{ .Values.externalDatabase.sslMode }}{{ end }}
{{- end }}

{{/* The user-supplied Secret with the password, or "" if the chart manages it. */}}
{{- define "track-record.databaseExistingSecret" -}}
{{- if .Values.database.enabled }}{{ .Values.database.auth.existingSecret }}{{ else }}{{ .Values.externalDatabase.existingSecret }}{{ end }}
{{- end }}

{{/* Name of the Secret the server reads the password from. */}}
{{- define "track-record.databaseSecretName" -}}
{{- $existing := include "track-record.databaseExistingSecret" . }}
{{- if $existing }}{{ $existing }}{{ else }}{{ include "track-record.component" (list . "database") }}{{ end }}
{{- end }}

{{- define "track-record.databaseSecretKey" -}}
{{- if include "track-record.databaseExistingSecret" . }}
{{- if .Values.database.enabled }}{{ .Values.database.auth.secretKey }}{{ else }}{{ .Values.externalDatabase.secretKey }}{{ end }}
{{- else }}password{{ end }}
{{- end }}

{{/*
The password when the chart manages it: the value, else the one in the
Secret of an earlier install, else a new random one. Computed once per
render and remembered, so the Secret and the hook Secret agree.
*/}}
{{- define "track-record.databasePassword" -}}
{{- if not (hasKey .Values "_databasePassword") }}
{{- $password := "" }}
{{- if .Values.database.enabled }}
{{- $password = .Values.database.auth.password }}
{{- if not $password }}
{{- $secret := lookup "v1" "Secret" .Release.Namespace (include "track-record.component" (list . "database")) }}
{{- if and $secret $secret.data (hasKey $secret.data "password") }}
{{- $password = index $secret.data "password" | b64dec }}
{{- else }}
{{- $password = randAlphaNum 32 }}
{{- end }}
{{- end }}
{{- else }}
{{- $password = required "externalDatabase.password or externalDatabase.existingSecret is required" .Values.externalDatabase.password }}
{{- end }}
{{- $_ := set .Values "_databasePassword" $password }}
{{- end }}
{{- index .Values "_databasePassword" }}
{{- end }}

{{/*
libpq/sqlx connection variables. The server and track-record-migrate read
the same PG* variables, so the password never goes into a URL (no escaping).
Call with (list $ "secret-name").
*/}}
{{- define "track-record.databaseEnv" -}}
{{- $root := index . 0 -}}
- name: PGHOST
  value: {{ include "track-record.databaseHost" $root | quote }}
- name: PGPORT
  value: {{ include "track-record.databasePort" $root | quote }}
- name: PGDATABASE
  value: {{ include "track-record.databaseName" $root | quote }}
- name: PGUSER
  value: {{ include "track-record.databaseUser" $root | quote }}
- name: PGPASSWORD
  valueFrom:
    secretKeyRef:
      name: {{ index . 1 }}
      key: {{ include "track-record.databaseSecretKey" $root }}
- name: PGSSLMODE
  value: {{ include "track-record.databaseSslMode" $root | quote }}
{{- end }}

{{/* Scheduling and pod settings shared by all workloads. Call with a component's values. */}}
{{- define "track-record.scheduling" -}}
{{- with .nodeSelector }}
nodeSelector:
  {{- toYaml . | nindent 2 }}
{{- end }}
{{- with .tolerations }}
tolerations:
  {{- toYaml . | nindent 2 }}
{{- end }}
{{- with .affinity }}
affinity:
  {{- toYaml . | nindent 2 }}
{{- end }}
{{- end }}

{{/* Renders a helper indented, or nothing (no blank line) if it is empty. Call with (list "helper" context indent). */}}
{{- define "track-record.optional" -}}
{{- $out := include (index . 0) (index . 1) | trim }}
{{- if $out }}
{{- $out | nindent (index . 2) }}
{{- end }}
{{- end }}

{{/* Init container that waits until the database accepts connections. */}}
{{- define "track-record.waitForDatabase" -}}
{{- $root := index . 0 -}}
- name: wait-for-database
  image: {{ include "track-record.image" (list $root $root.Values.database.image) }}
  imagePullPolicy: {{ $root.Values.database.image.pullPolicy }}
  command:
    - sh
    - -c
    - until pg_isready --quiet --timeout=5; do echo "waiting for $PGHOST:$PGPORT"; sleep 2; done
  env:
    {{- include "track-record.databaseEnv" (list $root (index . 1)) | nindent 4 }}
  securityContext:
    {{- toYaml $root.Values.migrations.securityContext | nindent 4 }}
{{- end }}
