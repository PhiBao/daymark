# Daymark web (Next.js). NEXT_PUBLIC_API_URL must be reachable from the browser.
FROM node:22-slim AS build
RUN corepack enable
WORKDIR /app
COPY web/package.json web/pnpm-lock.yaml* ./
RUN pnpm install --frozen-lockfile || pnpm install
COPY web/ ./
ARG NEXT_PUBLIC_API_URL=http://localhost:8787
ENV NEXT_PUBLIC_API_URL=${NEXT_PUBLIC_API_URL}
RUN pnpm build

FROM node:22-slim
RUN corepack enable
WORKDIR /app
COPY --from=build /app ./
EXPOSE 3000
CMD ["pnpm", "start", "-p", "3000"]
