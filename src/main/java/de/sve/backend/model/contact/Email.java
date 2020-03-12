package de.sve.backend.model.contact;

import java.util.List;

import javax.annotation.Nullable;

import com.google.auto.value.AutoValue;
import com.ryanharter.auto.value.gson.GenerateTypeAdapter;

@AutoValue
@GenerateTypeAdapter
public abstract class Email {

	public static Email create(MessageType type, String to, String subject, String content, List<Attachment> attachments) {
		return new AutoValue_Email(type, to, subject, content, attachments);
	}

	public abstract MessageType type();

	public abstract String to();

	public abstract String subject();

	public abstract String content();

	@Nullable
	public abstract List<Attachment> attachments();

	@AutoValue
	@GenerateTypeAdapter
	public static abstract class Attachment {

		public static Attachment create(String name, String mimeType, String data) {
			return new AutoValue_Email_Attachment(name, mimeType, data);
		}

		public abstract String name();

		public abstract String mimeType();

		public abstract String data();

	}

}
