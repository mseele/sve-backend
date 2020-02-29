package de.sve.backend.model.contact;

import com.google.auto.value.AutoValue;
import com.ryanharter.auto.value.gson.GenerateTypeAdapter;

@AutoValue
@GenerateTypeAdapter
public abstract class Message {

	public static Message create(MessageType type, String to, String name, String email, String phone, String message) {
		return new AutoValue_Message(type, to, name,  email,  phone,  message);
	}

	public abstract MessageType type();

	public abstract String to();

	public abstract String name();

	public abstract String email();

	public abstract String phone();

	public abstract String message();

}
