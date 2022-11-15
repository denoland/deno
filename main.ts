import { NestFactory } from "npm:@nestjs/core@9.2.0";
import { Controller, Get, Module } from "npm:@nestjs/common@9.2.0";

@Controller()
class AppController {
  @Get()
  index(): string {
    return "Hello World!";
  }
}

@Module({
  controllers: [AppController],
})
class AppModule {}

const app = await NestFactory.create(AppModule);
await app.listen(3000);
