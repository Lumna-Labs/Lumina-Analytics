FROM node:24-slim AS builder
WORKDIR /build

COPY package.json package-lock.json ./
RUN npm ci
COPY . .
# API base URL is baked in at build time (Vite inlines import.meta.env.*);
# override via --build-arg when the API isn't reachable at localhost:8080.
ARG VITE_API_BASE_URL=http://localhost:8080
ENV VITE_API_BASE_URL=$VITE_API_BASE_URL
RUN npm run build

FROM nginx:1.27-alpine AS runtime
COPY --from=builder /build/dist /usr/share/nginx/html
COPY nginx.conf /etc/nginx/conf.d/default.conf
EXPOSE 80
