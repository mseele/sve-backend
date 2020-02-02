package de.sve.backend;

import java.io.IOException;
import java.time.LocalDateTime;

import com.google.gson.Gson;
import com.google.gson.GsonBuilder;
import com.google.gson.TypeAdapter;
import com.google.gson.stream.JsonReader;
import com.google.gson.stream.JsonToken;
import com.google.gson.stream.JsonWriter;
import com.ryanharter.auto.value.gson.GenerateTypeAdapter;

import io.mikael.urlbuilder.UrlBuilder;

public class Utils {

	public static String BASE_URL = "https://www.sv-eutingen.de"; //$NON-NLS-1$

	public static Gson gson() {
		return new GsonBuilder().registerTypeAdapterFactory(GenerateTypeAdapter.FACTORY)
								.registerTypeAdapter(LocalDateTime.class, new LocalDateTimeAdapter())
								.create();
	}

	public static final UrlBuilder urlBuilder() {
		return UrlBuilder.fromString(BASE_URL);
	}

	private static class LocalDateTimeAdapter extends TypeAdapter<LocalDateTime> {

		@Override
		public void write(final JsonWriter jsonWriter, final LocalDateTime date) throws IOException {
			if (date == null) {
				jsonWriter.nullValue();
			} else {
				jsonWriter.value(date.toString());
			}
		}

		@Override
		public LocalDateTime read(final JsonReader jsonReader) throws IOException {
			if (jsonReader.peek() == JsonToken.NULL) {
				jsonReader.nextNull();
				return null;
			}
			return LocalDateTime.parse(jsonReader.nextString());
		}

	}

}
